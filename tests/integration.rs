use facet::filter_versions_bytes;
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

    let result = filter_versions_bytes(input.as_bytes(), &allowlist).unwrap();
    let result_str = String::from_utf8(result).unwrap();

    // Check metadata is preserved
    assert!(result_str.starts_with("created_at: 2024-04-01T00:00:05Z\n---\n"));

    // Check filtered gems are present
    assert!(result_str.contains("rails"));
    assert!(result_str.contains("sinatra"));
    assert!(result_str.contains("active_model_serializers"));

    // Check excluded gems are absent
    assert!(!result_str.contains("activerecord"));
    assert!(!result_str.contains("openapi_first"));
    assert!(!result_str.contains("0mq"));

    // Verify last occurrence is used for rails (should have updated999888)
    assert!(result_str.contains("updated999888"));
    assert!(!result_str.contains("abc123def456"));

    // Verify last occurrence for active_model_serializers
    assert!(result_str.contains("0.9.11"));
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

    let result = filter_versions_bytes(input.as_bytes(), &allowlist).unwrap();
    let result_str = String::from_utf8(result).unwrap();

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

    let result = filter_versions_bytes(input.as_bytes(), &allowlist).unwrap();
    let result_str = String::from_utf8(result).unwrap();

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

    let result = filter_versions_bytes(input.as_bytes(), &allowlist).unwrap();
    let result_str = String::from_utf8(result).unwrap();

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

    let result = filter_versions_bytes(input.as_bytes(), &allowlist).unwrap();
    let result_str = String::from_utf8(result).unwrap();

    // Yanked version marker should be preserved
    assert!(result_str.contains("-7.0.0"));
}
