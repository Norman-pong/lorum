//! Integration tests for the `lorum init` command.

use std::process::Command;

use tempfile::TempDir;

fn lorum_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lorum"));
    cmd.current_dir(std::env::current_dir().unwrap());
    cmd
}

#[test]
fn init_creates_global_config() {
    let dir = TempDir::new().unwrap();
    let config_dir = dir.path().join(".config").join("lorum");
    std::fs::create_dir_all(&config_dir).unwrap();

    let mut cmd = lorum_bin();
    cmd.arg("init");
    cmd.arg("--yes");
    cmd.env("HOME", dir.path());
    cmd.env_remove("XDG_CONFIG_HOME");

    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "init failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let config_path = config_dir.join("config.yaml");
    assert!(config_path.exists(), "config.yaml should exist");
}

#[test]
fn init_local_creates_project_config() {
    let dir = TempDir::new().unwrap();

    let mut cmd = lorum_bin();
    cmd.arg("init");
    cmd.arg("--local");
    cmd.arg("--yes");
    cmd.current_dir(&dir);

    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "init --local failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let config_path = dir.path().join(".lorum").join("config.yaml");
    let gitignore_path = dir.path().join(".lorum").join(".gitignore");
    assert!(config_path.exists(), "config.yaml should exist");
    assert!(gitignore_path.exists(), ".gitignore should exist");

    let gitignore = std::fs::read_to_string(&gitignore_path).unwrap();
    assert_eq!(gitignore, "skills/\n");
}

#[test]
fn init_refuses_overwrite() {
    let dir = TempDir::new().unwrap();
    let config_dir = dir.path().join(".config").join("lorum");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.yaml");
    std::fs::write(&config_path, "mcp:\n").unwrap();

    let mut cmd = lorum_bin();
    cmd.arg("init");
    cmd.arg("--yes");
    cmd.env("HOME", dir.path());
    cmd.env_remove("XDG_CONFIG_HOME");

    let output = cmd.output().unwrap();
    assert!(
        !output.status.success(),
        "init should fail when config exists"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("config already exists"),
        "stderr should mention overwrite: {}",
        stderr
    );
}
