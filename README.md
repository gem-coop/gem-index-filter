# Facet

Fast filtering for RubyGems `versions` index files. Designed for memory-constrained environments like Fastly Compute edge workers.

## Features

- **Streaming parser**: Handles 20+ MB files with minimal memory footprint
- **Simple filtering**: If a gem is in the allowlist, all occurrences are included
- **Order preservation**: Maintains exact original order from the input file
- **Fast lookups**: O(1) allowlist checks using HashSet
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
2. **Filter**: For each line, check if gem name is in the allowlist
3. **Output**: Include all matching lines in original order

### Why This Works

The filtering is intentionally simple:
- All occurrences of allowlisted gems are preserved
- Order is maintained exactly as in the input
- Memory efficient - we only store filtered results
- Perfect for append-only versions files where gems may appear multiple times

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
