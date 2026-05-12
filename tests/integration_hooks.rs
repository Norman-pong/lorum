//! End-to-end integration tests for hooks CRUD and sync flow.

use lorum::config::{self, HookHandler, LorumConfig};

#[test]
fn hook_add_and_remove_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");

    // Save empty config.
    let cfg = LorumConfig::default();
    config::save_config(&config_path, &cfg).unwrap();

    // Add two hooks via the public config API.
    let mut cfg = config::load_config(&config_path).unwrap();
    cfg.hooks.events.insert(
        "pre-tool-use".into(),
        vec![HookHandler {
            matcher: "Bash".into(),
            command: "scripts/safety-check.sh".into(),
            timeout: Some(60),
            handler_type: None,
        }],
    );
    cfg.hooks.events.insert(
        "post-tool-use".into(),
        vec![HookHandler {
            matcher: "Write|Edit".into(),
            command: "cargo fmt".into(),
            timeout: None,
            handler_type: None,
        }],
    );
    config::save_config(&config_path, &cfg).unwrap();

    // Load back and verify.
    let loaded = config::load_config(&config_path).unwrap();
    assert_eq!(loaded.hooks.events.len(), 2);

    let pre = &loaded.hooks.events["pre-tool-use"];
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0].matcher, "Bash");
    assert_eq!(pre[0].command, "scripts/safety-check.sh");
    assert_eq!(pre[0].timeout, Some(60));

    let post = &loaded.hooks.events["post-tool-use"];
    assert_eq!(post.len(), 1);
    assert_eq!(post[0].matcher, "Write|Edit");
    assert_eq!(post[0].command, "cargo fmt");
}

#[test]
fn hooks_config_roundtrip_save_load() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");

    let mut cfg = LorumConfig::default();
    cfg.hooks.events.insert(
        "session-start".into(),
        vec![HookHandler {
            matcher: "*".into(),
            command: "echo 'session started'".into(),
            timeout: None,
            handler_type: Some("command".into()),
        }],
    );

    config::save_config(&config_path, &cfg).unwrap();
    let loaded = config::load_config(&config_path).unwrap();
    assert_eq!(loaded, cfg);
}

#[test]
fn hooks_merge_with_mcp_config() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");

    let mut cfg = LorumConfig::default();
    cfg.mcp.servers.insert(
        "test".into(),
        lorum::config::McpServer {
            command: "echo".into(),
            args: vec![],
            env: std::collections::BTreeMap::new(),
        },
    );
    cfg.hooks.events.insert(
        "pre-tool-use".into(),
        vec![HookHandler {
            matcher: "Bash".into(),
            command: "check.sh".into(),
            timeout: None,
            handler_type: None,
        }],
    );

    config::save_config(&config_path, &cfg).unwrap();
    let loaded = config::load_config(&config_path).unwrap();
    assert_eq!(loaded.mcp.servers.len(), 1);
    assert_eq!(loaded.hooks.events.len(), 1);
}
