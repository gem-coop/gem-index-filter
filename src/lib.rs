//! gem-index-filter - Fast streaming filter for RubyGems versions index
//!
//! This library provides efficient streaming filtering of the RubyGems
//! versions file (https://rubygems.org/versions). It's designed to run in memory-
//! constrained environments like edge workers while handling 20+ MB index files.
//!
//! # Key Features
//!
//! - **True streaming**: Processes files line-by-line with zero memory retention
//! - **Flexible filtering**: Allow mode, block mode, or passthrough (no filtering)
//! - **Order preservation**: Maintains exact original order from input file
//! - **Fast filtering**: Uses HashSet for O(1) gem name lookups
//! - **Version stripping**: Optionally replace version lists with `0` to reduce size
//! - **Digest computation**: Optionally compute checksums (SHA-256, SHA-512) of filtered output
//!
//! # Examples
//!
//! **Allow mode** - include only specific gems:
//!
//! ```no_run
//! use gem_index_filter::{filter_versions_streaming, FilterMode, VersionOutput};
//! use std::collections::HashSet;
//! use std::fs::File;
//!
//! let input = File::open("versions").unwrap();
//! let mut output = File::create("versions.filtered").unwrap();
//! let mut allowlist = HashSet::new();
//! allowlist.insert("rails");
//! allowlist.insert("sinatra");
//! filter_versions_streaming(input, &mut output, FilterMode::Allow(&allowlist), VersionOutput::Preserve, None).unwrap();
//! ```
//!
//! **Block mode** - exclude specific gems:
//!
//! ```no_run
//! # use gem_index_filter::{filter_versions_streaming, FilterMode, VersionOutput};
//! # use std::collections::HashSet;
//! # use std::fs::File;
//! let input = File::open("versions").unwrap();
//! let mut output = File::create("versions.filtered").unwrap();
//! let mut blocklist = HashSet::new();
//! blocklist.insert("big-gem");
//! filter_versions_streaming(input, &mut output, FilterMode::Block(&blocklist), VersionOutput::Preserve, None).unwrap();
//! ```
//!
//! **With digest computation**:
//!
//! ```no_run
//! # use gem_index_filter::{filter_versions_streaming, FilterMode, VersionOutput, DigestAlgorithm};
//! # use std::fs::File;
//! let input = File::open("versions").unwrap();
//! let mut output = File::create("versions.filtered").unwrap();
//! let digest = filter_versions_streaming(
//!     input,
//!     &mut output,
//!     FilterMode::Passthrough,
//!     VersionOutput::Preserve,
//!     Some(DigestAlgorithm::Sha256)
//! ).unwrap();
//! if let Some(checksum) = digest {
//!     println!("SHA-256: {}", checksum);
//! }
//! ```

pub mod filter;

pub use filter::{filter_versions_streaming, DigestAlgorithm, FilterMode, VersionOutput};
