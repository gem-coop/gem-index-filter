use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};

/// Stream and filter versions file by first word (gem name) with zero memory retention
///
/// This function:
/// - Reads input line by line
/// - Passes through metadata until "---" separator
/// - Checks first word (space-separated) against allowlist
/// - Immediately writes matching lines to output
/// - Ignores everything after the first word until newline
/// - Retains only the current line buffer in memory
pub fn filter_versions_streaming<R: Read, W: Write>(
    input: R,
    output: &mut W,
    allowlist: &HashSet<&str>,
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
                output.write_all(line.as_bytes())?;
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
        filter_versions_streaming(input.as_bytes(), &mut output, &allowlist).unwrap();

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
        filter_versions_streaming(input.as_bytes(), &mut output, &allowlist).unwrap();

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
        filter_versions_streaming(input.as_bytes(), &mut output, &allowlist).unwrap();

        let result = String::from_utf8(output).unwrap();

        // Should only contain metadata
        assert!(result.contains("created_at"));
        assert!(result.contains("---"));
        assert!(!result.contains("rails"));
        assert!(!result.contains("sinatra"));
    }
}
