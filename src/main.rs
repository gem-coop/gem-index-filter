use gem_index_filter::filter::filter_versions_streaming;
use gem_index_filter::{DigestAlgorithm, FilterMode, VersionOutput};
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Parse flags
    let version_output = if args.iter().any(|arg| arg == "--strip-versions") {
        VersionOutput::Strip
    } else {
        VersionOutput::Preserve
    };

    // Find --allow, --block, and --digest flags and extract their values
    let mut allowlist_file: Option<&str> = None;
    let mut blocklist_file: Option<&str> = None;
    let mut digest_algorithm: Option<DigestAlgorithm> = None;
    let mut i = 1; // Start after program name
    while i < args.len() {
        if args[i] == "--allow" {
            if i + 1 < args.len() {
                allowlist_file = Some(&args[i + 1]);
                i += 2;
            } else {
                eprintln!("Error: --allow requires a file path");
                std::process::exit(1);
            }
        } else if args[i] == "--block" {
            if i + 1 < args.len() {
                blocklist_file = Some(&args[i + 1]);
                i += 2;
            } else {
                eprintln!("Error: --block requires a file path");
                std::process::exit(1);
            }
        } else if args[i] == "--digest" {
            if i + 1 < args.len() {
                let algo_str = args[i + 1].to_lowercase();
                digest_algorithm = match algo_str.as_str() {
                    "sha256" | "sha-256" => Some(DigestAlgorithm::Sha256),
                    "sha512" | "sha-512" => Some(DigestAlgorithm::Sha512),
                    _ => {
                        eprintln!(
                            "Error: Unknown digest algorithm '{}'. Supported: sha256, sha512",
                            args[i + 1]
                        );
                        std::process::exit(1);
                    }
                };
                i += 2;
            } else {
                eprintln!("Error: --digest requires an algorithm (sha256, sha512)");
                std::process::exit(1);
            }
        } else {
            i += 1;
        }
    }

    // Get positional arguments (excluding program name and flags)
    let digest_arg = digest_algorithm
        .as_ref()
        .map(|_| {
            args.iter()
                .position(|a| a == "--digest")
                .and_then(|i| args.get(i + 1))
        })
        .flatten();

    let positional_args: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|arg| {
            *arg != "--strip-versions"
                && *arg != "--allow"
                && *arg != "--block"
                && *arg != "--digest"
                && !allowlist_file.map_or(false, |f| *arg == f)
                && !blocklist_file.map_or(false, |f| *arg == f)
                && !digest_arg.map_or(false, |d| *arg == d)
        })
        .collect();

    if positional_args.is_empty() {
        eprintln!("Usage: gem-index-filter [OPTIONS] <versions-file> [output-file]");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  <versions-file>   Path to the versions file (or - for stdin)");
        eprintln!("  [output-file]     Optional output file (defaults to stdout)");
        eprintln!();
        eprintln!("Options:");
        eprintln!(
            "  --allow <file>       Filter to only gems in allowlist file (one name per line)"
        );
        eprintln!("  --block <file>       Filter out gems in blocklist file (one name per line)");
        eprintln!("  --strip-versions     Replace version lists with '0' in output");
        eprintln!("  --digest <algorithm> Compute checksum of filtered output (sha256, sha512)");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  gem-index-filter versions.txt                                      # Pass through all gems");
        eprintln!("  gem-index-filter --allow allowlist.txt versions.txt filtered.txt   # Filter to allowlist");
        eprintln!("  gem-index-filter --block blocklist.txt versions.txt filtered.txt   # Block specific gems");
        eprintln!("  gem-index-filter --allow allow.txt --block block.txt versions.txt  # Allow mode with blocked gems removed");
        eprintln!(
            "  gem-index-filter --strip-versions versions.txt filtered.txt        # Strip versions"
        );
        eprintln!("  gem-index-filter --digest sha256 versions.txt filtered.txt         # Compute SHA-256 checksum");
        eprintln!(
            "  curl https://rubygems.org/versions | facet --allow allowlist.txt - > filtered.txt"
        );
        std::process::exit(1);
    }

    let versions_file = positional_args[0].as_str();
    let output_file = positional_args.get(1).map(|s| s.as_str());

    // Read filter lists if specified
    let allowlist_owned = allowlist_file.map(read_gem_list).transpose()?;
    let blocklist_owned = blocklist_file.map(read_gem_list).transpose()?;

    // Determine filter mode with preprocessing optimization:
    // If both allow and block are specified, preprocess by removing blocked gems from allowlist
    // This reduces to just 2 runtime modes: Allow or Block (or Passthrough)
    let filter_set_owned: Option<HashSet<String>> = match (allowlist_owned, blocklist_owned) {
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
            Some(allow)
        }
        (Some(allow), None) => {
            eprintln!("Loaded {} gems from allowlist", allow.len());
            Some(allow)
        }
        (None, Some(block)) => {
            eprintln!("Loaded {} gems from blocklist", block.len());
            Some(block)
        }
        (None, None) => None,
    };

    // Create the filter mode by converting String references to &str
    // Keep owned set and converted set separate to manage lifetimes
    let filter_set_refs: Option<HashSet<&str>> = filter_set_owned
        .as_ref()
        .map(|set| set.iter().map(|s| s.as_str()).collect());

    // Determine which mode to use based on what was specified
    let mode = match (&filter_set_refs, allowlist_file, blocklist_file) {
        (Some(set), Some(_), Some(_)) => FilterMode::Allow(set), // Both: use Allow with preprocessed set
        (Some(set), Some(_), None) => FilterMode::Allow(set),    // Allow only
        (Some(set), None, Some(_)) => FilterMode::Block(set),    // Block only
        _ => FilterMode::Passthrough,                            // Neither
    };

    // Open input
    let input: Box<dyn io::Read> = if versions_file == "-" {
        Box::new(io::stdin())
    } else {
        Box::new(File::open(versions_file)?)
    };

    // Stream and filter
    if let Some(output_path) = output_file {
        let mut output = File::create(output_path)?;
        let digest =
            filter_versions_streaming(input, &mut output, mode, version_output, digest_algorithm)?;
        eprintln!("Written to {}", output_path);
        if let Some(checksum) = digest {
            eprintln!("{}: {}", digest_algorithm.unwrap().name(), checksum);
        }
    } else {
        let mut output = io::stdout();
        let digest =
            filter_versions_streaming(input, &mut output, mode, version_output, digest_algorithm)?;
        if let Some(checksum) = digest {
            eprintln!("{}: {}", digest_algorithm.unwrap().name(), checksum);
        }
    }

    Ok(())
}

/// Read gem list from file (one gem name per line, supports comments with #)
fn read_gem_list(path: &str) -> io::Result<HashSet<String>> {
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
