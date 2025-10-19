use std::io::{BufRead, BufReader, Read};

/// Parse result containing metadata and gem entries
pub struct VersionsFile {
    pub metadata: String,
    pub entries: Vec<GemLine>,
}

/// A single gem line with position tracking
#[derive(Debug, Clone)]
pub struct GemLine {
    pub line_number: usize,
    pub name: String,
    pub content: String, // "[-]versions MD5" portion
}

/// Parse the versions file in a streaming fashion
///
/// The format is:
/// ```text
/// metadata lines
/// ---
/// gemname [-]version[,version]* MD5
/// ```
pub fn parse_versions<R: Read>(reader: R) -> std::io::Result<VersionsFile> {
    let mut buf_reader = BufReader::new(reader);
    let mut metadata = String::new();
    let mut line = String::new();

    // Read metadata until we hit the separator "---"
    loop {
        line.clear();
        let n = buf_reader.read_line(&mut line)?;
        if n == 0 {
            // EOF before separator - malformed file
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "No separator found in versions file",
            ));
        }

        if line.trim() == "---" {
            metadata.push_str(&line);
            break;
        }

        metadata.push_str(&line);
    }

    // Parse gem entries
    let mut entries = Vec::new();
    let mut line_number = metadata.lines().count() + 1; // Line after separator

    loop {
        line.clear();
        let n = buf_reader.read_line(&mut line)?;
        if n == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse line: "gemname [-]version[,version]* MD5"
        // Split on first space to get gemname
        if let Some((name, content)) = trimmed.split_once(' ') {
            entries.push(GemLine {
                line_number,
                name: name.to_string(),
                content: content.to_string(),
            });
        }

        line_number += 1;
    }

    Ok(VersionsFile { metadata, entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0,7.0.1 abc123
activerecord 7.0.0 def456
"#;

        let result = parse_versions(input.as_bytes()).unwrap();

        assert_eq!(result.metadata, "created_at: 2024-04-01T00:00:05Z\n---\n");
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].name, "rails");
        assert_eq!(result.entries[0].content, "7.0.0,7.0.1 abc123");
        assert_eq!(result.entries[1].name, "activerecord");
        assert_eq!(result.entries[1].content, "7.0.0 def456");
    }

    #[test]
    fn test_parse_yanked_version() {
        let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails -7.0.0,7.0.1 abc123
"#;

        let result = parse_versions(input.as_bytes()).unwrap();

        assert_eq!(result.entries[0].content, "-7.0.0,7.0.1 abc123");
    }
}
