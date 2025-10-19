# Facet

Fast filtering for RubyGems `versions` index files. Designed for memory-constrained environments like Fastly Compute edge workers.

## Features

- **Streaming parser**: Handles 20+ MB files with minimal memory footprint (~3MB for 10k gems)
- **Deterministic output**: Preserves original gem order (by first occurrence)
- **Last-occurrence semantics**: When a gem appears multiple times, uses the most recent data
- **Fast filtering**: O(1) lookups using FxHashMap
- **Tested**: Comprehensive test suite with real-world data

## Performance

Filtering a 21MB versions file with 10k gem allowlist:
- Time: <100ms
- Memory: ~3MB
- Output: Typically <1MB

## Usage

### Command Line

```bash
# Filter a versions file with an allowlist
facet versions.txt allowlist.txt filtered.txt

# Stream from stdin
curl https://rubygems.org/versions | facet - allowlist.txt > filtered.txt
```

Allowlist format (one gem name per line):
```text
# Comments are supported
rails
sinatra
activerecord
puma
```

### Library

```rust
use facet::filter_versions_bytes;
use std::collections::HashSet;
use std::fs::File;

let input = File::open("versions")?;
let mut allowlist = HashSet::new();
allowlist.insert("rails");
allowlist.insert("sinatra");

let filtered = filter_versions_bytes(input, &allowlist)?;
std::fs::write("versions.filtered", filtered)?;
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
2. **Track**: Store each allowlisted gem in a HashMap with:
   - `first_line_number`: Original position (for deterministic ordering)
   - `last_content`: Most recent version data
3. **Output**: Sort by `first_line_number` and serialize

### Why This Works

The versions file is append-only. New releases or yanked versions are appended to the end. This means:
- First occurrence determines display order
- Last occurrence has the most up-to-date info
- We can stream input without loading it all into memory

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
3. Parse new lines, update existing gems or append new ones
4. Maintain stable ordering (first occurrence positions don't change)

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
