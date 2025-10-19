use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};

/// Stream and filter versions file by first word (gem name) with zero memory retention
///
/// This function:
/// - Reads input line by line
/// - Passes through metadata until "---" separator
/// - Checks first word (space-separated) against allowlist
/// - Immediately writes matching lines to output
/// - Optionally strips version information, replacing with "0"
/// - Ignores everything after the first word until newline
/// - Retains only the current line buffer in memory
pub fn filter_versions_streaming<R: Read, W: Write>(
    input: R,
    output: &mut W,
    allowlist: &HashSet<&str>,
    strip_versions: bool,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(input);
    let mut line = String::new();

    // Pass through metadata until separator "---"
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

    // Filter gem entries by first word
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

        // Extract first word (gem name)
        if let Some(space_pos) = trimmed.find(' ') {
            let gem_name = &trimmed[..space_pos];
            if allowlist.contains(gem_name) {
                if strip_versions {
                    // Parse and reconstruct line: gemname versions md5 -> gemname 0 md5
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 3 {
                        writeln!(output, "{} 0 {}", parts[0], parts[2])?;
                    } else {
                        // Fallback for malformed lines
                        output.write_all(line.as_bytes())?;
                    }
                } else {
                    output.write_all(line.as_bytes())?;
                }
            }
        }
    }

    Ok(())
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
        filter_versions_streaming(input.as_bytes(), &mut output, &allowlist, false).unwrap();

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
        filter_versions_streaming(input.as_bytes(), &mut output, &allowlist, false).unwrap();

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
        filter_versions_streaming(input.as_bytes(), &mut output, &allowlist, false).unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should only contain metadata
        assert!(result.contains("created_at"));
        assert!(result.contains("---"));
        assert!(!result.contains("rails"));
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
        filter_versions_streaming(input.as_bytes(), &mut output, &allowlist, true).unwrap();

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
        filter_versions_streaming(input.as_bytes(), &mut output, &allowlist, true).unwrap();

        let result = String::from_utf8(output).unwrap();

        let lines: Vec<&str> = result.lines().collect();
        let gem_lines: Vec<&str> = lines.iter().skip(2).copied().collect();

        // Verify original order is preserved with stripped versions
        assert_eq!(gem_lines.len(), 3);
        assert_eq!(gem_lines[0], "zebra 0 aaa111");
        assert_eq!(gem_lines[1], "mango 0 ccc333");
        assert_eq!(gem_lines[2], "banana 0 ddd444");
    }
}
