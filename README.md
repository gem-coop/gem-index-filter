# gem-index-filter

Fast filtering for RubyGems `versions` index files. Designed for memory-constrained environments like Fastly Compute edge workers.

## Features

- **Streaming parser**: Handles 20+ MB files with minimal memory footprint
- **Flexible filtering**: Allow mode, block mode, or passthrough (no filtering)
- **Combined filters**: Use `--allow` and `--block` together (allowlist - blocklist)
- **Version stripping**: Optionally replace version lists with `0` to reduce size
- **Order preservation**: Maintains exact original order from the input file

## Usage

### HTTP Server

The server binary provides a webhook-based filtering service with two endpoints:

**Starting the server:**

```bash
# Build with server feature
cargo build --release --features server

# Configure via environment variables
export CACHE_PATH="/tmp/versions.filtered"
export ALLOWLIST_PATH="allowlist.txt"
export BLOCKLIST_PATH="blocklist.txt"  # optional
export PORT=3000  # optional, defaults to 3000

# Run the server
./target/release/gem-index-filter-server
```

**Endpoints:**

- `POST /webhook` - Fetches from https://rubygems.org/versions, applies filtering with version stripping, and caches the result
- `GET /versions` - Returns the cached filtered versions file

**Example usage:**

```bash
# Trigger regeneration of filtered versions file
curl -X POST http://localhost:3000/webhook

# Fetch the cached filtered versions
curl http://localhost:3000/versions
```

**Use case:** Deploy as a service that maintains an up-to-date filtered versions index. Trigger `/webhook` on a schedule (e.g., hourly cron job) or via external webhook, and serve the cached result to clients requesting `/versions`.

**Configuration:**

- `CACHE_PATH`: Where to store the filtered versions file (default: `/tmp/versions.filtered`)
- `ALLOWLIST_PATH`: Path to allowlist file (optional)
- `BLOCKLIST_PATH`: Path to blocklist file (optional)
- `PORT`: Server port (default: `3000`)

The server applies the same filtering logic as the CLI tool, automatically using:
- Passthrough mode if no filter lists provided
- Allow mode if only allowlist provided
- Block mode if only blocklist provided
- Combined mode (allowlist - blocklist) if both provided

**Note:** The server always strips versions (replaces with `0`) to reduce cache size.

### Command Line

```bash
gem-index-filter [OPTIONS] <versions-file> [output-file]

Options:
  --allow <file>    Filter to only gems in allowlist file (one name per line)
  --block <file>    Filter out gems in blocklist file (one name per line)
  --strip-versions  Replace version lists with '0' in output
```

**Examples:**

```bash
# Pass through all gems (no filtering)
gem-index-filter versions

# Filter to only gems in allowlist
gem-index-filter --allow allowlist.txt versions filtered.txt

# Block specific gems
gem-index-filter --block blocklist.txt versions filtered.txt

# Allow mode with blocked gems removed (allowlist - blocklist)
gem-index-filter --allow allow.txt --block block.txt versions filtered.txt

# Strip version information (replace with '0')
gem-index-filter --strip-versions versions filtered.txt

# Stream from stdin
curl https://rubygems.org/versions | gem-index-filter --allow allowlist.txt - > filtered.txt
```

**Filter file format** (one gem name per line, `#` for comments):

```text
rails
sinatra
activerecord
puma
```

### Library

```rust
use gem_index_filter::{filter_versions_streaming, FilterMode};
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
- **Order preservation**: Maintains exact original order from input
- **All occurrences preserved**: versions is append-only

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

# Build CLI binary
cargo build --release

# Build server binary
cargo build --release --features server

# For Fastly Compute (wasm32-wasi target)
cargo build --target wasm32-wasi --release
```

## Testing

```bash
# Run all tests
cargo test

# Test with real data (if you have a versions file)
cargo run --release -- versions output.txt
```

## License

MIT
