//! Facet - Fast streaming filter for RubyGems versions index
//!
//! This library provides efficient streaming filtering of the RubyGems
//! versions file (https://rubygems.org/versions). It's designed to run in memory-
//! constrained environments like edge workers while handling 20+ MB index files.
//!
//! # Key Features
//!
//! - **True streaming**: Processes files line-by-line with zero memory retention
//! - **Simple filtering**: Matches first word (gem name) against allowlist
//! - **Order preservation**: Maintains exact original order from input file
//! - **Fast filtering**: Uses HashSet for O(1) allowlist lookups
//!
//! # Example
//!
//! ```no_run
//! use facet::filter_versions_streaming;
//! use std::collections::HashSet;
//! use std::fs::File;
//!
//! let input = File::open("versions").unwrap();
//! let mut output = File::create("versions.filtered").unwrap();
//! let mut allowlist = HashSet::new();
//! allowlist.insert("rails");
//! allowlist.insert("sinatra");
//!
//! filter_versions_streaming(input, &mut output, &allowlist, false).unwrap();
//! ```

pub mod filter;

pub use filter::filter_versions_streaming;
