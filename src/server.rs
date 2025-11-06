use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use gem_index_filter::{filter_versions_streaming, FilterMode, VersionOutput};
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tokio::fs;

/// Server configuration
#[derive(Clone)]
struct AppState {
    cache_path: PathBuf,
    // Preprocessed filter mode (created once at startup)
    filter_mode: FilterMode<'static>,
}

#[tokio::main]
async fn main() {
    // Parse configuration from environment variables
    let cache_path = env::var("CACHE_PATH")
        .unwrap_or_else(|_| "/tmp/versions.filtered".to_string())
        .into();

    let allowlist_path = env::var("ALLOWLIST_PATH").ok();
    let blocklist_path = env::var("BLOCKLIST_PATH").ok();

    // Load filter lists
    let allowlist = allowlist_path.and_then(|path| read_gem_list(&path).ok());
    let blocklist = blocklist_path.and_then(|path| read_gem_list(&path).ok());

    // Preprocess filter mode at startup (optimization from CLAUDE.md)
    // Create FilterMode<'static> once by leaking memory - acceptable for long-running server
    let filter_mode = match (allowlist, blocklist) {
        (Some(mut allow), Some(block)) => {
            // Optimization: allowlist - blocklist, then use Allow mode
            let original_count = allow.len();
            allow.retain(|gem| !block.contains(gem));
            eprintln!(
                "Loaded {} gems from allowlist, {} from blocklist ({} gems after removing blocked)",
                original_count,
                block.len(),
                allow.len()
            );
            // Leak the owned HashSet first to get 'static lifetime
            let leaked: &'static HashSet<String> = Box::leak(Box::new(allow));
            // Now create references with 'static lifetime
            let refs: HashSet<&'static str> = leaked.iter().map(|s| s.as_str()).collect();
            FilterMode::Allow(Box::leak(Box::new(refs)))
        }
        (Some(allow), None) => {
            eprintln!("Loaded {} gems from allowlist", allow.len());
            let leaked: &'static HashSet<String> = Box::leak(Box::new(allow));
            let refs: HashSet<&'static str> = leaked.iter().map(|s| s.as_str()).collect();
            FilterMode::Allow(Box::leak(Box::new(refs)))
        }
        (None, Some(block)) => {
            eprintln!("Loaded {} gems from blocklist", block.len());
            let leaked: &'static HashSet<String> = Box::leak(Box::new(block));
            let refs: HashSet<&'static str> = leaked.iter().map(|s| s.as_str()).collect();
            FilterMode::Block(Box::leak(Box::new(refs)))
        }
        (None, None) => {
            eprintln!("No filter lists specified - using passthrough mode");
            FilterMode::Passthrough
        }
    };

    let state = AppState {
        cache_path,
        filter_mode,
    };

    // Build router with two endpoints
    let app = Router::new()
        .route("/webhook", post(webhook_handler))
        .route("/versions", get(versions_handler))
        .with_state(state);

    // Get port from environment or use default
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let addr = format!("0.0.0.0:{}", port);
    eprintln!("Starting server on {}", addr);
    eprintln!("Endpoints:");
    eprintln!("  POST /webhook  - Trigger version file regeneration");
    eprintln!("  GET  /versions - Serve cached filtered versions file");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// POST /webhook - Trigger regeneration of filtered versions file
async fn webhook_handler(State(state): State<AppState>) -> Result<String, AppError> {
    eprintln!("Webhook triggered - fetching from rubygems.org/versions");

    // Fetch from rubygems.org
    let response = reqwest::get("https://rubygems.org/versions")
        .await
        .map_err(|e| AppError::FetchError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(AppError::FetchError(format!(
            "Failed to fetch versions: HTTP {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::FetchError(e.to_string()))?;

    eprintln!("Downloaded {} bytes from rubygems.org", bytes.len());

    // Create temporary file for output
    let temp_path = format!("{}.tmp", state.cache_path.display());
    let mut output_file = File::create(&temp_path)
        .map_err(|e| AppError::IoError(format!("Failed to create temp file: {}", e)))?;

    // Stream and filter with version stripping
    filter_versions_streaming(
        &bytes[..],
        &mut output_file,
        state.filter_mode,
        VersionOutput::Strip,
        None,
    )
    .map_err(|e| AppError::IoError(format!("Failed to filter versions: {}", e)))?;

    // Atomically replace cache file
    std::fs::rename(&temp_path, &state.cache_path)
        .map_err(|e| AppError::IoError(format!("Failed to update cache: {}", e)))?;

    eprintln!("Cache updated at {}", state.cache_path.display());

    Ok(format!(
        "Versions file regenerated and cached at {}",
        state.cache_path.display()
    ))
}

/// GET /versions - Serve cached filtered versions file
async fn versions_handler(State(state): State<AppState>) -> Result<Response, AppError> {
    // Check if cache file exists
    if !state.cache_path.exists() {
        return Err(AppError::NotFound(
            "Cache file not found. Trigger /webhook first.".to_string(),
        ));
    }

    // Read cached file
    let content = fs::read(&state.cache_path)
        .await
        .map_err(|e| AppError::IoError(format!("Failed to read cache: {}", e)))?;

    eprintln!("Serving cached file ({} bytes)", content.len());

    // Return as plain text with proper content type
    Ok((
        StatusCode::OK,
        [("content-type", "text/plain; charset=utf-8")],
        content,
    )
        .into_response())
}

/// Read gem list from file (one gem name per line, supports comments with #)
fn read_gem_list(path: &str) -> std::io::Result<HashSet<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut gems = HashSet::new();

    for line in reader.lines() {
        let line = line?;
        let gem_name = line.trim();
        // Skip empty lines and comments
        if !gem_name.is_empty() && !gem_name.starts_with('#') {
            gems.insert(gem_name.to_string());
        }
    }

    Ok(gems)
}

/// Application errors
#[derive(Debug)]
enum AppError {
    FetchError(String),
    IoError(String),
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::FetchError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::IoError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        (status, message).into_response()
    }
}
