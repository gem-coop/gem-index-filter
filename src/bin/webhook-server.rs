use axum::{http::StatusCode, response::IntoResponse, routing::post, Router};
use aws_sdk_s3::Client as S3Client;
use gem_index_filter::{filter_versions_streaming, DigestAlgorithm, FilterMode, VersionOutput};
use serde::Serialize;
use std::collections::HashSet;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

#[derive(Clone)]
struct AppState {
    s3_client: S3Client,
    active_tasks: Arc<Mutex<JoinSet<()>>>,
    bucket_name: String,
    allowlist_key: String,
}

#[derive(Serialize)]
struct AcceptedResponse {
    status: String,
}

#[tokio::main]
async fn main() {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;
    let s3_client = S3Client::new(&config);

    let state = AppState {
        s3_client,
        active_tasks: Arc::new(Mutex::new(JoinSet::new())),
        bucket_name: std::env::var("BUCKET_NAME").unwrap_or("rubygems-filtered".to_string()),
        allowlist_key: std::env::var("ALLOWLIST_KEY")
            .unwrap_or("allowlist.txt".to_string()),
    };

    let app = Router::new()
        .route("/webhook", post(handle_webhook))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();

    println!("Listening on 0.0.0.0:8080");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state.active_tasks))
        .await
        .unwrap();
}

async fn handle_webhook(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let s3_client = state.s3_client.clone();
    let bucket_name = state.bucket_name.clone();
    let allowlist_key = state.allowlist_key.clone();

    state.active_tasks.lock().await.spawn(async move {
        if let Err(e) = process_index(s3_client, bucket_name, allowlist_key).await {
            eprintln!("Error processing index: {}", e);
        }
    });

    (
        StatusCode::ACCEPTED,
        axum::Json(AcceptedResponse {
            status: "accepted".to_string(),
        }),
    )
}

async fn process_index(
    s3_client: S3Client,
    bucket_name: String,
    allowlist_key: String,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Fetching allowlist from S3: {}/{}", bucket_name, allowlist_key);

    // Fetch allowlist from S3
    let allowlist = fetch_allowlist(&s3_client, &bucket_name, &allowlist_key).await?;
    println!("Loaded {} gems in allowlist", allowlist.len());

    println!("Fetching RubyGems index from https://index.rubygems.org/versions");

    // Fetch the RubyGems index
    let response = reqwest::get("https://index.rubygems.org/versions")
        .await?
        .bytes()
        .await?;

    println!("Downloaded {} bytes, filtering...", response.len());

    // Filter the gem index using the existing library
    let (filtered_data, checksum) = filter_gem_index(&response, &allowlist)?;

    println!(
        "Filtered to {} bytes, SHA-256: {}",
        filtered_data.len(),
        checksum
    );

    // Upload filtered data with timestamp
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let data_key = format!("versions/filtered-{}.bin", timestamp);

    s3_client
        .put_object()
        .bucket(&bucket_name)
        .key(&data_key)
        .body(filtered_data.into())
        .content_type("application/octet-stream")
        .send()
        .await?;

    // Upload checksum as metadata file
    let checksum_key = format!("versions/filtered-{}.sha256", timestamp);
    s3_client
        .put_object()
        .bucket(&bucket_name)
        .key(&checksum_key)
        .body(checksum.into_bytes().into())
        .content_type("text/plain")
        .send()
        .await?;

    // Update "latest" pointers
    let latest_data_key = "versions/filtered-latest.bin";
    let latest_checksum_key = "versions/filtered-latest.sha256";

    // Copy the timestamped versions to the latest pointers
    s3_client
        .copy_object()
        .bucket(&bucket_name)
        .copy_source(format!("{}/{}", bucket_name, data_key))
        .key(latest_data_key)
        .send()
        .await?;

    s3_client
        .copy_object()
        .bucket(&bucket_name)
        .copy_source(format!("{}/{}", bucket_name, checksum_key))
        .key(latest_checksum_key)
        .send()
        .await?;

    println!(
        "Uploaded: {} and {} (also updated latest pointers)",
        data_key, checksum_key
    );
    Ok(())
}

/// Fetch and parse allowlist from S3
async fn fetch_allowlist(
    s3_client: &S3Client,
    bucket_name: &str,
    key: &str,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let response = s3_client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await?;

    let bytes = response.body.collect().await?.into_bytes();
    let content = String::from_utf8(bytes.to_vec())?;

    let mut allowlist = HashSet::new();
    for line in content.lines() {
        let gem_name = line.trim();
        // Skip empty lines and comments
        if !gem_name.is_empty() && !gem_name.starts_with('#') {
            allowlist.insert(gem_name.to_string());
        }
    }

    Ok(allowlist)
}

/// Filter gem index using the existing gem-index-filter library
fn filter_gem_index(
    data: &[u8],
    allowlist: &HashSet<String>,
) -> Result<(Vec<u8>, String), Box<dyn std::error::Error>> {
    // Convert HashSet<String> to HashSet<&str> for FilterMode
    let allowlist_refs: HashSet<&str> = allowlist.iter().map(|s| s.as_str()).collect();

    // Create input reader from bytes
    let input = Cursor::new(data);

    // Create output buffer
    let mut output = Vec::new();

    // Stream and filter with SHA-256 checksum computation
    let checksum = filter_versions_streaming(
        input,
        &mut output,
        FilterMode::Allow(&allowlist_refs),
        VersionOutput::Strip, // Strip versions to reduce output size
        Some(DigestAlgorithm::Sha256),
    )?;

    Ok((output, checksum.unwrap_or_default()))
}

async fn shutdown_signal(active_tasks: Arc<Mutex<JoinSet<()>>>) {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");

    println!("Shutdown signal received, waiting for active tasks...");

    let mut tasks = active_tasks.lock().await;
    while tasks.join_next().await.is_some() {}

    println!("All tasks completed");
}
