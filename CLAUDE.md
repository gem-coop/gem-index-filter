# gem-index-filter - Agent Instructions

This document provides project-specific context for AI agents and developers working on gem-index-filter.

## Project Purpose

gem-index-filter is a high-performance streaming filter for RubyGems versions index files. It's designed to run in **memory-constrained environments** like Fastly Compute edge workers, processing 20+ MB files with minimal memory footprint.

## Core Design Principles

### 1. Streaming-First Architecture

**Never buffer entire files in memory.** All operations must be line-by-line streaming.

- Use `BufReader` and process one line at a time
- Write output immediately, don't accumulate results
- The only data retained in memory: current line buffer and filter sets
- Target: Process 20MB files in ~3MB memory

### 2. Performance-Critical Hot Paths

This tool processes hundreds of thousands of lines per run. Small inefficiencies compound.

**Optimize by:**
- **Hoisting conditionals outside loops** - Check filter mode once before entering the loop, not on every iteration
- **Specializing code paths** - Prefer separate optimized loops over nested conditionals
- **Preprocessing data structures** - Combine filter operations at startup (e.g., `allowlist - blocklist`) rather than checking both at runtime
- **Eliminating branches** - Use enum variants and match expressions outside hot loops

**Example of what NOT to do:**
```rust
// BAD: Checks mode on every iteration
for line in lines {
    match mode {
        Allow => if allowlist.contains(gem) { ... }
        Block => if !blocklist.contains(gem) { ... }
    }
}
```

**Example of what TO do:**
```rust
// GOOD: Check mode once, then optimized loop
match mode {
    Allow => {
        for line in lines {
            if allowlist.contains(gem) { ... }
        }
    }
    Block => {
        for line in lines {
            if !blocklist.contains(gem) { ... }
        }
    }
}
```

### 3. Zero-Copy Where Possible

- Use string slices (`&str`) instead of owned strings when filtering
- Convert owned collections (`HashSet<String>`) to borrowed references (`&HashSet<&str>`) once at startup
- Use `#[inline]` on helper functions to enable compiler optimization

## File Format: RubyGems Versions Index

Understanding this format is critical for correct parsing:

```
created_at: 2024-04-01T00:00:05Z
---
gemname version[,version]* MD5hash
gemname version[,version]* MD5hash
```

**Format rules:**
- Metadata section (before `---` separator) must be passed through unchanged
- Each gem line: space-separated fields `name versions hash [extra...]`
- Versions can be comma-separated: `rails 7.0.0,7.0.1,7.0.2 abc123`
- Yanked versions prefixed with `-`: `gemname -0.9.0,0.9.1 hash`
- There may be additional fields after the hash - these must be preserved
- Gems can appear **multiple times** (append-only file)
- When filtering, preserve ALL occurrences of matching gems
- When stripping versions, replace version field with `0` while preserving all other fields: `gemname versions hash extra` → `gemname 0 hash extra`

## Architecture Patterns

### FilterMode Enum

The `FilterMode` enum eliminates runtime mode checking:

```rust
pub enum FilterMode<'a> {
    Passthrough,              // No filtering
    Allow(&'a HashSet<&'a str>),  // Include only these gems
    Block(&'a HashSet<&'a str>),  // Exclude these gems
}
```

**Why three modes when we could have four?**
When both `--allow` and `--block` are specified, we preprocess at startup:
```rust
effective_allowlist = allowlist - blocklist
mode = Allow(effective_allowlist)
```

This reduces runtime to just 2 code paths (Allow or Block) while supporting 3 user-facing modes.

### Helper Functions

Extract common logic but preserve performance:

```rust
#[inline]
fn write_gem_line_stripped<W: Write>(trimmed: &str, output: &mut W) -> std::io::Result<()> {
    // Common logic for version stripping
}
```

The `#[inline]` attribute allows the compiler to optimize away function call overhead.

## Testing

### Running Tests

**Must source cargo environment first:**
```bash
. ~/.cargo/env && cargo test
```

Cargo is not in the default PATH in this environment.

### Test Organization

- **Unit tests**: In `src/filter.rs` under `#[cfg(test)]`
- **Integration tests**: In `tests/integration.rs`
- **Doc tests**: In `src/lib.rs` code examples

### Test Data Requirements

Always use realistic versions file format in tests:

```rust
let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0,7.0.1 abc123
sinatra 3.0.0 def456
"#;
```

**Test coverage should include:**
- Metadata preservation
- Order preservation (gems appear in original order)
- Duplicate gem handling (all occurrences preserved)
- Yanked versions (`-` prefix)
- Edge cases (empty lists, malformed lines)
- Combined filter modes

## Code Style

### Comments

Document **why**, not **what**:

```rust
// GOOD: Explains the optimization
// Hoist allowlist check outside the loop for efficiency
match mode { ... }

// BAD: Describes what the code does
// Check if we're in allow mode
match mode { ... }
```

### Error Handling

- Use `std::io::Result` for I/O operations
- Fail fast on malformed input (missing `---` separator is an error)
- Provide clear error messages for CLI errors

### Naming Conventions

- Use meaningful enum/type names: `FilterMode` not `Mode`
- Prefer explicit over clever: `strip_versions` not `strip`
- Use full words: `allowlist` not `allow_list` or `al`

## Common Patterns

### Reading Filter Lists

```rust
fn read_gem_list(path: &str) -> io::Result<HashSet<String>> {
    // Read file line by line
    // Skip empty lines and comments (lines starting with #)
    // Return HashSet for O(1) lookups
}
```

### Converting to References

```rust
let owned: HashSet<String> = read_gem_list("file.txt")?;
let refs: HashSet<&str> = owned.iter().map(|s| s.as_str()).collect();
let mode = FilterMode::Allow(&refs);
```

### Streaming Pattern

```rust
let mut reader = BufReader::new(input);
let mut line = String::new();

loop {
    line.clear();  // Reuse buffer
    let n = reader.read_line(&mut line)?;
    if n == 0 { break; }  // EOF

    // Process line...
    output.write_all(line.as_bytes())?;
}
```

### Version Stripping Pattern

When stripping versions, preserve all fields except the version field (index 1):

```rust
// Input:  "gemname 1.0.0,2.0.0 abc123 extra1 extra2"
// Output: "gemname 0 abc123 extra1 extra2"

let parts: Vec<&str> = trimmed.split_whitespace().collect();
if parts.len() >= 3 {
    write!(output, "{} 0", parts[0])?;  // gemname + 0
    for part in &parts[2..] {            // all fields after version
        write!(output, " {}", part)?;
    }
    writeln!(output)?;
}
```

This ensures any extra metadata fields are preserved during version stripping.

## Performance Expectations

From README.md:

- **Input**: 21MB versions file
- **Allowlist**: 10,000 gems
- **Time**: <100ms
- **Memory**: ~3MB
- **Output**: Typically <1MB

Maintain or improve these benchmarks when making changes.

## When Adding New Features

1. **Consider memory impact** - Will this require buffering data?
2. **Profile hot paths** - Does this add checks inside the main loop?
3. **Preprocess when possible** - Can we compute this once at startup?
4. **Add comprehensive tests** - Cover edge cases and realistic data
5. **Update documentation** - CLI help text, README examples, doc comments

## Related Files

- **Core logic**: `src/filter.rs` (streaming filter implementation)
- **CLI**: `src/main.rs` (argument parsing, mode determination)
- **Library API**: `src/lib.rs` (public exports and doc examples)
- **Integration tests**: `tests/integration.rs`
- **Documentation**: `README.md`
