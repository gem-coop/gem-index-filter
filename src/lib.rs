//! Facet - Fast filtering for RubyGems versions index
//!
//! This library provides efficient streaming parsing and filtering of the RubyGems
//! versions file (https://rubygems.org/versions). It's designed to run in memory-
//! constrained environments like edge workers while handling 20+ MB index files.
//!
//! # Key Features
//!
//! - **Streaming parser**: Processes large files without loading them entirely into memory
//! - **Deterministic output**: Preserves original gem order (by first occurrence)
//! - **Last-occurrence semantics**: Uses the most recent data when gems appear multiple times
//! - **Fast filtering**: Uses FxHashMap for O(1) lookups
//!
//! # Example
//!
//! ```no_run
//! use facet::filter_versions_bytes;
//! use std::collections::HashSet;
//! use std::fs::File;
//!
//! let input = File::open("versions").unwrap();
//! let mut allowlist = HashSet::new();
//! allowlist.insert("rails");
//! allowlist.insert("sinatra");
//!
//! let filtered = filter_versions_bytes(input, &allowlist).unwrap();
//! std::fs::write("versions.filtered", filtered).unwrap();
//! ```

pub mod filter;
pub mod parser;

use std::collections::HashSet;
use std::io::Read;

pub use filter::{filter_versions, FilteredVersions};
pub use parser::{parse_versions, GemLine, VersionsFile};

/// High-level API: Filter versions file and return bytes
///
/// This is the primary entry point for filtering. It:
/// 1. Parses the input stream
/// 2. Filters by allowlist
/// 3. Serializes to bytes
///
/// # Arguments
///
/// * `input` - Any readable source (file, network stream, etc.)
/// * `allowlist` - Set of gem names to include
///
/// # Returns
///
/// Filtered versions file as bytes, ready to write or serve
pub fn filter_versions_bytes<R: Read>(
    input: R,
    allowlist: &HashSet<&str>,
) -> std::io::Result<Vec<u8>> {
    let versions = parse_versions(input)?;
    let filtered = filter_versions(versions, allowlist);
    Ok(filtered.to_bytes())
}

/// High-level API: Filter versions file and return string
///
/// Same as `filter_versions_bytes` but returns a String for convenience.
pub fn filter_versions_string<R: Read>(
    input: R,
    allowlist: &HashSet<&str>,
) -> std::io::Result<String> {
    let versions = parse_versions(input)?;
    let filtered = filter_versions(versions, allowlist);
    Ok(filtered.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_versions_bytes() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
activerecord 7.0.0 def456
sinatra 3.0.0 ghi789
"#;

        let mut allowlist = HashSet::new();
        allowlist.insert("rails");
        allowlist.insert("sinatra");

        let result = filter_versions_bytes(input.as_bytes(), &allowlist).unwrap();
        let result_str = String::from_utf8(result).unwrap();

        assert!(result_str.contains("rails 7.0.0 abc123"));
        assert!(result_str.contains("sinatra 3.0.0 ghi789"));
        assert!(!result_str.contains("activerecord"));
    }
}
