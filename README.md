# Facet

Fast filtering for RubyGems `versions` index files. Designed for memory-constrained environments like Fastly Compute edge workers.

## Features

- **Streaming parser**: Handles 20+ MB files with minimal memory footprint
- **Flexible filtering**: Allow mode, block mode, or passthrough (no filtering)
- **Combined filters**: Use `--allow` and `--block` together (allowlist - blocklist)
- **Version stripping**: Optionally replace version lists with `0` to reduce size
- **Order preservation**: Maintains exact original order from the input file
- **Fast lookups**: O(1) gem name checks using HashSet
- **Tested**: Comprehensive test suite with real-world data

## Performance

Filtering a 21MB versions file with 10k gem allowlist:
- Time: <100ms
- Memory: ~3MB
- Output: Typically <1MB

## Usage

### Command Line

```bash
facet [OPTIONS] <versions-file> [output-file]

Options:
  --allow <file>    Filter to only gems in allowlist file (one name per line)
  --block <file>    Filter out gems in blocklist file (one name per line)
  --strip-versions  Replace version lists with '0' in output
```

**Examples:**

```bash
# Pass through all gems (no filtering)
facet versions.txt

# Filter to only gems in allowlist
facet --allow allowlist.txt versions.txt filtered.txt

# Block specific gems
facet --block blocklist.txt versions.txt filtered.txt

# Allow mode with blocked gems removed (allowlist - blocklist)
facet --allow allow.txt --block block.txt versions.txt filtered.txt

# Strip version information (replace with '0')
facet --strip-versions versions.txt filtered.txt

# Stream from stdin
curl https://rubygems.org/versions | facet --allow allowlist.txt - > filtered.txt
```

**Filter file format** (one gem name per line, `#` for comments):

```text
rails
sinatra
activerecord
puma
# This is a comment
```

### Library

```rust
use facet::{filter_versions_streaming, FilterMode};
use std::collections::HashSet;
use std::fs::File;

let input = File::open("versions")?;
let mut output = File::create("versions.filtered")?;

// Create allowlist
let mut allowlist = HashSet::new();
allowlist.insert("rails");
allowlist.insert("sinatra");

// Stream and filter
filter_versions_streaming(input, &mut output, FilterMode::Allow(&allowlist), false)?;
```

**Other modes:**

```rust
// Block mode - exclude specific gems
let mut blocklist = HashSet::new();
blocklist.insert("big-gem");
filter_versions_streaming(input, &mut output, FilterMode::Block(&blocklist), false)?;

// Passthrough mode - no filtering
filter_versions_streaming(input, &mut output, FilterMode::Passthrough, false)?;

// Strip versions while filtering
filter_versions_streaming(input, &mut output, FilterMode::Allow(&allowlist), true)?;
```

## Versions File Format

The format uses one line per rubygem, with additional lines appended for updates:

```text
created_at: 2024-04-01T00:00:05Z
---
gemname [-]version[,version]* MD5
```

- **gemname**: The name of the rubygem
- **versions**: Comma-separated list of versions (may include platform)
- **-**: Minus prefix indicates yanked version
- **MD5**: Hash of the gem's "info" file

When a gem appears multiple times, the last occurrence has the authoritative MD5.

## How It Works

1. **Parse**: Stream input line-by-line using BufReader
2. **Filter**: Based on mode, check gem name against filter list:
   - **Passthrough**: Include all gems (no filtering)
   - **Allow mode**: Include only gems where `gemlist.contains(gemname) == true`
   - **Block mode**: Include only gems where `gemlist.contains(gemname) == false`
   - **Combined**: Preprocess `allowlist - blocklist` at startup, then use Allow mode
3. **Output**: Write matching lines immediately in original order

### Design Principles

The filtering is optimized for performance and simplicity:
- **Streaming architecture**: Only current line buffer held in memory
- **Mode hoisting**: Filter mode checked once before loop, not per-line
- **Unified filtering**: Single `process_filtered()` function handles both Allow/Block modes
- **Order preservation**: Maintains exact original order from input
- **All occurrences preserved**: Perfect for append-only files where gems appear multiple times

## Future: Incremental Updates

The versions file supports HTTP range requests, enabling incremental updates:

```rust
// Future API design
struct FilteredIndex {
    data: Vec<u8>,
    last_byte_offset: u64,  // Track where we've processed to
}

impl FilteredIndex {
    fn update(&mut self, range_data: &[u8]) {
        // Process only new appended data
        // Merge updates into existing filtered index
    }
}
```

**Strategy:**
1. Store byte offset we've processed to
2. Fetch `Range: bytes={offset}-` for incremental updates
3. Filter new lines and append matching gems to existing filtered index
4. All occurrences preserved - simple append operation

## Building

```bash
# Run tests
cargo test

# Build release binary
cargo build --release

# For Fastly Compute (wasm32-wasi target)
cargo build --target wasm32-wasi --release
```

## Testing

```bash
# Run all tests
cargo test

# Test with real data (if you have a versions file)
cargo run --release -- versions.txt test_allowlist.txt output.txt
```

## License

MIT
