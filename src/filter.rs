use crate::parser::{GemLine, VersionsFile};
use rustc_hash::FxHashMap;
use std::collections::HashSet;

/// Entry tracking first occurrence and last content
#[derive(Debug)]
struct GemEntry {
    first_line_number: usize,
    last_content: String,
}

/// Filter versions file by allowlist, preserving original order and using last occurrence
///
/// When a gem appears multiple times:
/// - Position is determined by first occurrence
/// - Content (versions + MD5) is taken from last occurrence
pub fn filter_versions(versions: VersionsFile, allowlist: &HashSet<&str>) -> FilteredVersions {
    let mut gems: FxHashMap<String, GemEntry> = FxHashMap::default();

    // Process all entries, tracking first position and last content
    for entry in versions.entries {
        if allowlist.contains(entry.name.as_str()) {
            gems.entry(entry.name.clone())
                .and_modify(|e| {
                    // Update to last occurrence content
                    e.last_content = entry.content.clone();
                })
                .or_insert(GemEntry {
                    first_line_number: entry.line_number,
                    last_content: entry.content,
                });
        }
    }

    // Convert to sorted vec by original position
    let mut sorted_gems: Vec<_> = gems
        .into_iter()
        .map(|(name, entry)| GemLine {
            line_number: entry.first_line_number,
            name,
            content: entry.last_content,
        })
        .collect();

    sorted_gems.sort_by_key(|g| g.line_number);

    FilteredVersions {
        metadata: versions.metadata,
        entries: sorted_gems,
    }
}

/// Filtered versions ready for output
pub struct FilteredVersions {
    pub metadata: String,
    pub entries: Vec<GemLine>,
}

impl FilteredVersions {
    /// Serialize to bytes in the original format
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(self.metadata.as_bytes());

        for entry in &self.entries {
            result.extend_from_slice(entry.name.as_bytes());
            result.push(b' ');
            result.extend_from_slice(entry.content.as_bytes());
            result.push(b'\n');
        }

        result
    }

    /// Serialize to string
    pub fn to_string(&self) -> String {
        let mut result = self.metadata.clone();

        for entry in &self.entries {
            result.push_str(&entry.name);
            result.push(' ');
            result.push_str(&entry.content);
            result.push('\n');
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::VersionsFile;

    #[test]
    fn test_filter_preserves_order() {
        let versions = VersionsFile {
            metadata: "created_at: 2024-04-01T00:00:05Z\n---\n".to_string(),
            entries: vec![
                GemLine {
                    line_number: 1,
                    name: "rails".to_string(),
                    content: "7.0.0 abc123".to_string(),
                },
                GemLine {
                    line_number: 2,
                    name: "activerecord".to_string(),
                    content: "7.0.0 def456".to_string(),
                },
                GemLine {
                    line_number: 3,
                    name: "sinatra".to_string(),
                    content: "3.0.0 ghi789".to_string(),
                },
            ],
        };

        let mut allowlist = HashSet::new();
        allowlist.insert("sinatra");
        allowlist.insert("rails");

        let filtered = filter_versions(versions, &allowlist);

        assert_eq!(filtered.entries.len(), 2);
        assert_eq!(filtered.entries[0].name, "rails"); // First in original
        assert_eq!(filtered.entries[1].name, "sinatra"); // Second in original
    }

    #[test]
    fn test_filter_uses_last_occurrence() {
        let versions = VersionsFile {
            metadata: "created_at: 2024-04-01T00:00:05Z\n---\n".to_string(),
            entries: vec![
                GemLine {
                    line_number: 1,
                    name: "rails".to_string(),
                    content: "7.0.0 abc123".to_string(),
                },
                GemLine {
                    line_number: 2,
                    name: "activerecord".to_string(),
                    content: "7.0.0 def456".to_string(),
                },
                GemLine {
                    line_number: 3,
                    name: "rails".to_string(),
                    content: "7.0.1 xyz999".to_string(), // Updated version
                },
            ],
        };

        let mut allowlist = HashSet::new();
        allowlist.insert("rails");

        let filtered = filter_versions(versions, &allowlist);

        assert_eq!(filtered.entries.len(), 1);
        assert_eq!(filtered.entries[0].line_number, 1); // Position from first
        assert_eq!(filtered.entries[0].content, "7.0.1 xyz999"); // Content from last
    }

    #[test]
    fn test_to_string_format() {
        let filtered = FilteredVersions {
            metadata: "created_at: 2024-04-01T00:00:05Z\n---\n".to_string(),
            entries: vec![GemLine {
                line_number: 1,
                name: "rails".to_string(),
                content: "7.0.0 abc123".to_string(),
            }],
        };

        let output = filtered.to_string();
        assert_eq!(
            output,
            "created_at: 2024-04-01T00:00:05Z\n---\nrails 7.0.0 abc123\n"
        );
    }
}
