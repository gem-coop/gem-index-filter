use gem_index_filter::{filter_versions_streaming, FilterMode, VersionOutput};
use std::collections::HashSet;

/// Test with realistic versions file format including duplicates and yanked versions
#[test]
fn test_realistic_filtering() {
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
-A 0.0.0 8b1527991f0022e46140907a7fc4cfd4
.cat 0.0.1 631fd60a806eaf5026c86fff3155c289
.omghi 1,2 7a67c0434100c2ab635b9f4865ee86bd
0mq 0.1.0,0.1.1,0.1.2,0.2.0,0.2.1,0.3.0,0.4.0,0.4.1,0.5.0,0.5.1,0.5.2,0.5.3 6146193f8f7e944156b0b42ec37bad3e
rails 7.0.0,7.0.1,7.0.2 abc123def456
activerecord 7.0.0,7.0.1 fed456cba321
sinatra 3.0.0,3.0.1 123456789abc
active_model_serializers -0.9.10 7ad37af4aec8cc089e409e1fdec86f3d
active_model_serializers 0.9.11 a6d40e97b289ee6c806e5e9f7031623b
openapi_first 1.4.1 40fbfdebcbfee3863df697e1d641f637
rails 7.0.3,7.0.4 updated999888
"#;

    let mut allowlist = HashSet::new();
    allowlist.insert("rails");
    allowlist.insert("sinatra");
    allowlist.insert("active_model_serializers");

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&allowlist),
        VersionOutput::Preserve,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    // Check metadata is preserved
    assert!(result_str.starts_with("created_at: 2024-04-01T00:00:05Z\n---\n"));

    // Check excluded gems are absent
    assert!(!result_str.contains("activerecord"));
    assert!(!result_str.contains("openapi_first"));
    assert!(!result_str.contains("0mq"));

    // All occurrences should be preserved
    let lines: Vec<&str> = result_str.lines().skip(2).collect();
    assert_eq!(lines.len(), 5); // rails (2x), sinatra (1x), active_model_serializers (2x)

    // Verify both rails occurrences are present
    assert!(result_str.contains("rails 7.0.0,7.0.1,7.0.2 abc123def456"));
    assert!(result_str.contains("rails 7.0.3,7.0.4 updated999888"));

    // Verify both active_model_serializers occurrences are present
    assert!(
        result_str.contains("active_model_serializers -0.9.10 7ad37af4aec8cc089e409e1fdec86f3d")
    );
    assert!(result_str.contains("active_model_serializers 0.9.11 a6d40e97b289ee6c806e5e9f7031623b"));

    // Verify sinatra is present
    assert!(result_str.contains("sinatra 3.0.0,3.0.1 123456789abc"));

    // Verify order is preserved (rails, sinatra, active_model_serializers, active_model_serializers, rails)
    assert_eq!(lines[0], "rails 7.0.0,7.0.1,7.0.2 abc123def456");
    assert_eq!(lines[1], "sinatra 3.0.0,3.0.1 123456789abc");
    assert_eq!(
        lines[2],
        "active_model_serializers -0.9.10 7ad37af4aec8cc089e409e1fdec86f3d"
    );
    assert_eq!(
        lines[3],
        "active_model_serializers 0.9.11 a6d40e97b289ee6c806e5e9f7031623b"
    );
    assert_eq!(lines[4], "rails 7.0.3,7.0.4 updated999888");
}

#[test]
fn test_order_preservation() {
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
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&allowlist),
        VersionOutput::Preserve,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    // Split into lines and find gem entries
    let lines: Vec<&str> = result_str.lines().collect();
    let gem_lines: Vec<&str> = lines.iter().skip(2).copied().collect(); // Skip metadata

    // Verify original order is preserved (zebra, mango, banana)
    assert_eq!(gem_lines.len(), 3);
    assert!(gem_lines[0].starts_with("zebra"));
    assert!(gem_lines[1].starts_with("mango"));
    assert!(gem_lines[2].starts_with("banana"));
}

#[test]
fn test_empty_allowlist() {
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
sinatra 3.0.0 def456
"#;

    let allowlist = HashSet::new();

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&allowlist),
        VersionOutput::Preserve,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    // Should only have metadata
    assert_eq!(result_str, "created_at: 2024-04-01T00:00:05Z\n---\n");
}

#[test]
fn test_all_gems_allowed() {
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
sinatra 3.0.0 def456
"#;

    let mut allowlist = HashSet::new();
    allowlist.insert("rails");
    allowlist.insert("sinatra");

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&allowlist),
        VersionOutput::Preserve,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    assert!(result_str.contains("rails 7.0.0 abc123"));
    assert!(result_str.contains("sinatra 3.0.0 def456"));
}

#[test]
fn test_yanked_versions_preserved() {
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails -7.0.0,7.0.1 abc123
"#;

    let mut allowlist = HashSet::new();
    allowlist.insert("rails");

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&allowlist),
        VersionOutput::Preserve,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    // Yanked version marker should be preserved
    assert!(result_str.contains("-7.0.0"));
}

#[test]
fn test_duplicate_gems_all_preserved() {
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 1.0.0 aaa111
other_gem 1.0.0 bbb222
rails 2.0.0 ccc333
another_gem 1.0.0 ddd444
rails 3.0.0 eee555
"#;

    let mut allowlist = HashSet::new();
    allowlist.insert("rails");

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&allowlist),
        VersionOutput::Preserve,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    let lines: Vec<&str> = result_str.lines().skip(2).collect();

    // All 3 occurrences of rails should be present
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "rails 1.0.0 aaa111");
    assert_eq!(lines[1], "rails 2.0.0 ccc333");
    assert_eq!(lines[2], "rails 3.0.0 eee555");
}

#[test]
fn test_strip_versions_integration() {
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0,7.0.1,7.0.2 abc123def456
activerecord 7.0.0,7.0.1 fed456cba321
sinatra 3.0.0,3.0.1 123456789abc
rails 7.0.3,7.0.4 updated999888
"#;

    let mut allowlist = HashSet::new();
    allowlist.insert("rails");
    allowlist.insert("sinatra");

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&allowlist),
        VersionOutput::Strip,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    // Check metadata is preserved
    assert!(result_str.starts_with("created_at: 2024-04-01T00:00:05Z\n---\n"));

    // Check stripped versions
    assert!(result_str.contains("rails 0 abc123def456"));
    assert!(result_str.contains("rails 0 updated999888"));
    assert!(result_str.contains("sinatra 0 123456789abc"));

    // Check original versions are gone
    assert!(!result_str.contains("7.0.0,7.0.1,7.0.2"));
    assert!(!result_str.contains("7.0.3,7.0.4"));
    assert!(!result_str.contains("3.0.0,3.0.1"));

    // Check excluded gem is absent
    assert!(!result_str.contains("activerecord"));

    // Verify both rails occurrences are present with stripped versions
    let lines: Vec<&str> = result_str.lines().skip(2).collect();
    assert_eq!(lines.len(), 3); // rails (2x), sinatra (1x)
    assert_eq!(lines[0], "rails 0 abc123def456");
    assert_eq!(lines[1], "sinatra 0 123456789abc");
    assert_eq!(lines[2], "rails 0 updated999888");
}

#[test]
fn test_strip_versions_with_yanked() {
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
active_model_serializers -0.9.10,0.9.11 7ad37af4aec8cc089e409e1fdec86f3d
rails 7.0.0,7.0.1 abc123
"#;

    let mut allowlist = HashSet::new();
    allowlist.insert("rails");
    allowlist.insert("active_model_serializers");

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&allowlist),
        VersionOutput::Strip,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    // Stripped versions should replace yanked versions too
    assert!(result_str.contains("active_model_serializers 0 7ad37af4aec8cc089e409e1fdec86f3d"));
    assert!(result_str.contains("rails 0 abc123"));

    // Original version info should be gone
    assert!(!result_str.contains("-0.9.10"));
    assert!(!result_str.contains("0.9.11"));
    assert!(!result_str.contains("7.0.0,7.0.1"));
}

#[test]
fn test_block_mode_integration() {
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0,7.0.1,7.0.2 abc123def456
activerecord 7.0.0,7.0.1 fed456cba321
sinatra 3.0.0,3.0.1 123456789abc
puma 5.0.0 xyz999
rails 7.0.3,7.0.4 updated999888
"#;

    let mut blocklist = HashSet::new();
    blocklist.insert("activerecord");
    blocklist.insert("puma");

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Block(&blocklist),
        VersionOutput::Preserve,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    // Check metadata is preserved
    assert!(result_str.starts_with("created_at: 2024-04-01T00:00:05Z\n---\n"));

    // Check non-blocked gems are present
    assert!(result_str.contains("rails 7.0.0,7.0.1,7.0.2 abc123def456"));
    assert!(result_str.contains("rails 7.0.3,7.0.4 updated999888"));
    assert!(result_str.contains("sinatra 3.0.0,3.0.1 123456789abc"));

    // Check blocked gems are absent
    assert!(!result_str.contains("activerecord"));
    assert!(!result_str.contains("puma"));
}

#[test]
fn test_combined_allow_and_block_preprocessed() {
    // This test simulates what main.rs does: preprocess allowlist - blocklist
    let input = r#"created_at: 2024-04-01T00:00:05Z
---
rails 7.0.0 abc123
activerecord 7.0.0 def456
sinatra 3.0.0 ghi789
puma 5.0.0 xyz999
rack 2.0.0 aaa111
"#;

    // Simulate: allowlist = {rails, activerecord, sinatra, puma}
    // blocklist = {activerecord, puma}
    // Result: effective_allowlist = {rails, sinatra}
    let mut effective_allowlist = HashSet::new();
    effective_allowlist.insert("rails");
    effective_allowlist.insert("activerecord");
    effective_allowlist.insert("sinatra");
    effective_allowlist.insert("puma");

    let blocklist = vec!["activerecord", "puma"];
    effective_allowlist.retain(|gem| !blocklist.contains(&gem.as_ref()));

    let mut output = Vec::new();
    filter_versions_streaming(
        input.as_bytes(),
        &mut output,
        FilterMode::Allow(&effective_allowlist),
        VersionOutput::Preserve,
        None,
    )
    .unwrap();
    let result_str = String::from_utf8(output).unwrap();

    // Should contain only rails and sinatra (allowlisted but not blocked)
    assert!(result_str.contains("rails 7.0.0 abc123"));
    assert!(result_str.contains("sinatra 3.0.0 ghi789"));

    // Should NOT contain blocked gems (even though they were in allowlist)
    assert!(!result_str.contains("activerecord"));
    assert!(!result_str.contains("puma"));

    // Should NOT contain gems not in allowlist
    assert!(!result_str.contains("rack"));
}
