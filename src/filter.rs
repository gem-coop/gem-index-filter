use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};
use sha2::{Sha256, Sha512, Digest};

/// Filtering mode for gem selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode<'a> {
    /// Pass through all gems (no filtering)
    Passthrough,
    /// Include only gems in the allowlist
    Allow(&'a HashSet<&'a str>),
    /// Exclude gems in the blocklist
    Block(&'a HashSet<&'a str>),
}

/// Version output mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOutput {
    /// Preserve original version information
    Preserve,
    /// Strip versions, replacing with '0'
    Strip,
}

/// Supported digest algorithms for checksum computation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestAlgorithm {
    /// SHA-256 checksum
    Sha256,
    /// SHA-512 checksum
    Sha512,
}

/// Internal enum for holding active digest state
enum DigestState {
    Sha256(Sha256),
    Sha512(Sha512),
}

/// Writer wrapper that computes digest of data as it's written
/// This enables streaming checksum computation with zero buffering
pub struct DigestWriter<'a, W: Write> {
    inner: &'a mut W,
    state: DigestState,
}

impl<'a, W: Write> DigestWriter<'a, W> {
    /// Create a new DigestWriter with the specified algorithm
    pub fn new(inner: &'a mut W, algorithm: DigestAlgorithm) -> Self {
        let state = match algorithm {
            DigestAlgorithm::Sha256 => DigestState::Sha256(Sha256::new()),
            DigestAlgorithm::Sha512 => DigestState::Sha512(Sha512::new()),
        };
        DigestWriter { inner, state }
    }

    /// Finalize the digest and return the hex-encoded checksum
    pub fn finalize(self) -> String {
        match self.state {
            DigestState::Sha256(hasher) => hex::encode(hasher.finalize()),
            DigestState::Sha512(hasher) => hex::encode(hasher.finalize()),
        }
    }
}

impl<'a, W: Write> Write for DigestWriter<'a, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Update digest with the data
        match &mut self.state {
            DigestState::Sha256(hasher) => hasher.update(buf),
            DigestState::Sha512(hasher) => hasher.update(buf),
        }
        // Write to underlying writer
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
/// Stream and filter versions file by first word (gem name) with zero memory retention
///
/// This function:
/// - Reads input line by line
/// - Passes through metadata until "---" separator
/// - Applies filtering based on mode (Allow/Block/Passthrough)
/// - Immediately writes matching lines to output
/// - Optionally strips version information, replacing with "0"
/// - Optionally computes a checksum of the filtered output
/// - Ignores everything after the first word until newline
/// - Retains only the current line buffer in memory
///
/// Returns:
/// - `Ok(None)` if no digest algorithm was specified
/// - `Ok(Some(hex_string))` if digest was computed
pub fn filter_versions_streaming<R: Read, W: Write>(
    input: R,
    output: &mut W,
    mode: FilterMode,
version_output: VersionOutput,
    digest_algorithm: Option<DigestAlgorithm>,
) -> std::io::Result<Option<String>> {
    let mut reader = BufReader::new(input);

// Wrap output in DigestWriter if checksum is requested
    match digest_algorithm {
        Some(algorithm) => {
            // Wrap output writer to compute digest as data streams through
            let mut digest_writer = DigestWriter::new(output, algorithm);

            // Pass through metadata until separator "---"
            pass_through_metadata(&mut reader, &mut digest_writer)?;

            // Branch to specialized filter function based on mode
            // This hoists the mode check outside the hot loop for performance
            match mode {
                FilterMode::Passthrough => process_passthrough(&mut reader, &mut digest_writer, version_output)?,
                FilterMode::Allow(allowlist) => process_filtered(&mut reader, &mut digest_writer, allowlist, true, version_output)?,
                FilterMode::Block(blocklist) => process_filtered(&mut reader, &mut digest_writer, blocklist, false, version_output)?,
            }

            // Finalize digest and return hex string
            Ok(Some(digest_writer.finalize()))
        }
        None => {
            // No digest requested, use output directly
            // Pass through metadata until separator "---"
            pass_through_metadata(&mut reader, output)?;

            // Branch to specialized filter function based on mode
            match mode {
                FilterMode::Passthrough => process_passthrough(&mut reader, output, version_output)?,
                FilterMode::Allow(allowlist) => process_filtered(&mut reader, output, allowlist, true, version_output)?,
                FilterMode::Block(blocklist) => process_filtered(&mut reader, output, blocklist, false, version_output)?,
            }

            Ok(None)
        }
    }
}

/// Pass through metadata lines until the "---" separator
fn pass_through_metadata<R: Read, W: Write>(
    reader: &mut BufReader<R>,
    output: &mut W,
) -> std::io::Result<()> {
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "No separator found in versions file",
            ));
        }

        output.write_all(line.as_bytes())?;

        if line.trim() == "---" {
            break;
        }
    }

    Ok(())
}

/// Process all gems without filtering
fn process_passthrough<R: Read, W: Write>(
    reader: &mut BufReader<R>,
    output: &mut W,
    version_output: VersionOutput,
) -> std::io::Result<()> {
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match version_output {
            VersionOutput::Strip => write_gem_line_stripped(trimmed, output)?,
            VersionOutput::Preserve => output.write_all(line.as_bytes())?,
        }
    }

    Ok(())
}

/// Process gems with filtering based on gemlist membership
///
/// When `include_on_match` is true (Allow mode): includes gems where gemlist.contains(gemname) == true
/// When `include_on_match` is false (Block mode): includes gems where gemlist.contains(gemname) == false
fn process_filtered<R: Read, W: Write>(
    reader: &mut BufReader<R>,
    output: &mut W,
    gemlist: &HashSet<&str>,
    include_on_match: bool,
    version_output: VersionOutput,
) -> std::io::Result<()> {
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Extract first word (gem name) and check gemlist
        if let Some(gem_name) = extract_gem_name(trimmed) {
            let is_in_list = gemlist.contains(gem_name);
            if is_in_list == include_on_match {
                write_gem_line(trimmed, &line, output, version_output)?;
            }
        }
    }

    Ok(())
}

/// Extract gem name (first word) from a gem line
#[inline]
fn extract_gem_name(line: &str) -> Option<&str> {
    line.find(' ').map(|space_pos| &line[..space_pos])
}

/// Write a gem line to output, optionally stripping version information
#[inline]
fn write_gem_line<W: Write>(
    trimmed: &str,
    original_line: &str,
    output: &mut W,
    version_output: VersionOutput,
) -> std::io::Result<()> {
    match version_output {
        VersionOutput::Strip => write_gem_line_stripped(trimmed, output),
        VersionOutput::Preserve => output.write_all(original_line.as_bytes()),
    }
}

/// Helper function to write a gem line with stripped version info
#[inline]
fn write_gem_line_stripped<W: Write>(trimmed: &str, output: &mut W) -> std::io::Result<()> {
    // Parse and reconstruct line: gemname versions md5 [extra...] -> gemname 0 md5 [extra...]
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() >= 3 {
        // Write: gemname 0 md5 [any additional fields]
        write!(output, "{} 0", parts[0])?;
        for part in &parts[2..] {
            write!(output, " {}", part)?;
        }
        writeln!(output)
    } else {
        // Fallback for malformed lines - write as-is with newline
        writeln!(output, "{}", trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_filter() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
activerecord 7.0.0 def456
sinatra 3.0.0 ghi789
rails 7.0.1 xyz999
"#;

        let mut allowlist = HashSet::new();
        allowlist.insert("rails");
        allowlist.insert("sinatra");

        let mut output = Vec::new();
let digest = filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Allow(&allowlist), VersionOutput::Preserve, None).unwrap();
        assert!(digest.is_none());

        let result = String::from_utf8(output).unwrap();

        // Should contain metadata
        assert!(result.contains("created_at: 2024-04-01T00:00:05Z"));
        assert!(result.contains("---"));

        // Should contain allowlisted gems
        assert!(result.contains("rails 7.0.0 abc123"));
        assert!(result.contains("sinatra 3.0.0 ghi789"));
        assert!(result.contains("rails 7.0.1 xyz999"));

        // Should NOT contain filtered gem
        assert!(!result.contains("activerecord"));
    }

    #[test]
    fn test_streaming_preserves_exact_format() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
"#;

        let mut allowlist = HashSet::new();
        allowlist.insert("rails");

        let mut output = Vec::new();
filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Allow(&allowlist), VersionOutput::Preserve, None).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert_eq!(result, input); // Should be identical for all-included case
    }

    #[test]
    fn test_streaming_empty_allowlist() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
sinatra 3.0.0 ghi789
"#;

        let allowlist = HashSet::new();

        let mut output = Vec::new();
filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Allow(&allowlist), VersionOutput::Preserve, None).unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should only contain metadata
        assert!(result.contains("created_at"));
        assert!(result.contains("---"));
        assert!(!result.contains("rails"));
        assert!(!result.contains("sinatra"));
    }

    #[test]
    fn test_passthrough_mode() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
activerecord 7.0.0 def456
sinatra 3.0.0 ghi789
"#;

        let mut output = Vec::new();
filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Passthrough, VersionOutput::Preserve, None).unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should contain metadata
        assert!(result.contains("created_at: 2024-04-01T00:00:05Z"));
        assert!(result.contains("---"));

        // Should contain all gems
        assert!(result.contains("rails 7.0.0 abc123"));
        assert!(result.contains("activerecord 7.0.0 def456"));
        assert!(result.contains("sinatra 3.0.0 ghi789"));
    }

    #[test]
    fn test_block_mode() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
activerecord 7.0.0 def456
sinatra 3.0.0 ghi789
puma 5.0.0 xyz999
"#;

        let mut blocklist = HashSet::new();
        blocklist.insert("activerecord");
        blocklist.insert("puma");

        let mut output = Vec::new();
filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Block(&blocklist), VersionOutput::Preserve, None).unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should contain metadata
        assert!(result.contains("created_at: 2024-04-01T00:00:05Z"));
        assert!(result.contains("---"));

        // Should contain non-blocked gems
        assert!(result.contains("rails 7.0.0 abc123"));
        assert!(result.contains("sinatra 3.0.0 ghi789"));

        // Should NOT contain blocked gems
        assert!(!result.contains("activerecord"));
        assert!(!result.contains("puma"));
    }

    #[test]
    fn test_block_mode_with_strip_versions() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0,7.0.1 abc123
activerecord 7.0.0 def456
sinatra 3.0.0 ghi789
"#;

        let mut blocklist = HashSet::new();
        blocklist.insert("activerecord");

        let mut output = Vec::new();
filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Block(&blocklist), VersionOutput::Strip, None).unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should contain stripped versions for non-blocked gems
        assert!(result.contains("rails 0 abc123"));
        assert!(result.contains("sinatra 0 ghi789"));

        // Should NOT contain blocked gem
        assert!(!result.contains("activerecord"));
    }

    #[test]
    fn test_strip_versions_preserves_extra_fields() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123 extra1 extra2
sinatra 3.0.0 def456
puma 5.0.0 ghi789 extra_field
"#;

        let mut allowlist = HashSet::new();
        allowlist.insert("rails");
        allowlist.insert("puma");

        let mut output = Vec::new();
filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Allow(&allowlist), VersionOutput::Strip, None).unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should preserve extra fields after md5 hash
        assert!(result.contains("rails 0 abc123 extra1 extra2"));
        assert!(result.contains("puma 0 ghi789 extra_field"));

        // Should NOT contain filtered gem
        assert!(!result.contains("sinatra"));
    }

    #[test]
    fn test_strip_versions() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0,7.0.1,7.0.2 abc123def456
activerecord 7.0.0 def456
sinatra 3.0.0,3.0.1 123456789abc
rails 7.0.3,7.0.4 updated999888
"#;

        let mut allowlist = HashSet::new();
        allowlist.insert("rails");
        allowlist.insert("sinatra");

        let mut output = Vec::new();
filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Allow(&allowlist), VersionOutput::Strip, None).unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should contain metadata
        assert!(result.contains("created_at: 2024-04-01T00:00:05Z"));
        assert!(result.contains("---"));

        // Should contain stripped versions (0 instead of version list)
        assert!(result.contains("rails 0 abc123def456"));
        assert!(result.contains("rails 0 updated999888"));
        assert!(result.contains("sinatra 0 123456789abc"));

        // Should NOT contain original version strings
        assert!(!result.contains("7.0.0,7.0.1,7.0.2"));
        assert!(!result.contains("7.0.3,7.0.4"));
        assert!(!result.contains("3.0.0,3.0.1"));

        // Should NOT contain filtered gem
        assert!(!result.contains("activerecord"));
    }

    #[test]
    fn test_strip_versions_preserves_order() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
zebra 1.0.0 aaa111
apple 1.0.0 bbb222
mango 1.0.0 ccc333
banana 1.0.0 ddd444
"#;

        let mut allowlist = HashSet::new();
        allowlist.insert("banana");
        allowlist.insert("zebra");
        allowlist.insert("mango");

        let mut output = Vec::new();
filter_versions_streaming(input.as_bytes(), &mut output, FilterMode::Allow(&allowlist), VersionOutput::Strip, None).unwrap();

        let result = String::from_utf8(output).unwrap();

        let lines: Vec<&str> = result.lines().collect();
        let gem_lines: Vec<&str> = lines.iter().skip(2).copied().collect();

        // Verify original order is preserved with stripped versions
        assert_eq!(gem_lines.len(), 3);
        assert_eq!(gem_lines[0], "zebra 0 aaa111");
        assert_eq!(gem_lines[1], "mango 0 ccc333");
        assert_eq!(gem_lines[2], "banana 0 ddd444");
    }

    #[test]
    fn test_digest_sha256() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
sinatra 3.0.0 ghi789
"#;

        let mut allowlist = HashSet::new();
        allowlist.insert("rails");

        let mut output = Vec::new();
        let digest = filter_versions_streaming(
            input.as_bytes(),
            &mut output,
            FilterMode::Allow(&allowlist),
            VersionOutput::Preserve,
            Some(DigestAlgorithm::Sha256)
        ).unwrap();

        // Should return a digest
        assert!(digest.is_some());
        let digest_hex = digest.unwrap();

        // SHA-256 produces 64 hex characters (32 bytes)
        assert_eq!(digest_hex.len(), 64);

        // Verify digest is hex
        assert!(digest_hex.chars().all(|c| c.is_ascii_hexdigit()));

        // Verify output content is correct
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("rails 7.0.0 abc123"));
        assert!(!result.contains("sinatra"));
    }


    #[test]
    fn test_digest_sha512() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
"#;

        let mut output = Vec::new();
        let digest = filter_versions_streaming(
            input.as_bytes(),
            &mut output,
            FilterMode::Passthrough,
            VersionOutput::Preserve,
            Some(DigestAlgorithm::Sha512)
        ).unwrap();

        // Should return a digest
        assert!(digest.is_some());
        let digest_hex = digest.unwrap();

        // SHA-512 produces 128 hex characters (64 bytes)
        assert_eq!(digest_hex.len(), 128);

        // Verify digest is hex
        assert!(digest_hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_digest_with_strip_versions() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0,7.0.1,7.0.2 abc123
sinatra 3.0.0 def456
"#;

        let mut allowlist = HashSet::new();
        allowlist.insert("rails");

        let mut output = Vec::new();
        let digest = filter_versions_streaming(
            input.as_bytes(),
            &mut output,
            FilterMode::Allow(&allowlist),
            VersionOutput::Strip,
            Some(DigestAlgorithm::Sha256)
        ).unwrap();

        assert!(digest.is_some());
        let result = String::from_utf8(output).unwrap();

        // Verify stripped output
        assert!(result.contains("rails 0 abc123"));

        // Digest should be different from non-stripped version
        let mut output2 = Vec::new();
        let digest2 = filter_versions_streaming(
            input.as_bytes(),
            &mut output2,
            FilterMode::Allow(&allowlist),
            VersionOutput::Preserve,
            Some(DigestAlgorithm::Sha256)
        ).unwrap();

        assert_ne!(digest.unwrap(), digest2.unwrap());
    }

    #[test]
    fn test_digest_consistency() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
"#;

        // Run twice with same input, should get same digest
        let mut output1 = Vec::new();
        let digest1 = filter_versions_streaming(
            input.as_bytes(),
            &mut output1,
            FilterMode::Passthrough,
            VersionOutput::Preserve,
            Some(DigestAlgorithm::Sha256)
        ).unwrap();

        let mut output2 = Vec::new();
        let digest2 = filter_versions_streaming(
            input.as_bytes(),
            &mut output2,
            FilterMode::Passthrough,
            VersionOutput::Preserve,
            Some(DigestAlgorithm::Sha256)
        ).unwrap();

        assert_eq!(digest1, digest2);
        assert_eq!(output1, output2);
    }
}
