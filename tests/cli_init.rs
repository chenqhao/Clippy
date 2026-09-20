use assert_cmd::Command;
use std::fs;

#[test]
fn init_creates_config_and_identity() {
    // Create a temporary directory that gets automatically deleted when `dir` is dropped.
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    // Run our binary with UCLIP_HOME pointing to the temp dir.
    // This prevents the test from touching your real config files!
    Command::cargo_bin("uclip")
        .expect("binary not found")
        .arg("init")
        .env("UCLIP_HOME", dir.path())
        .assert()
        .success(); // exit code 0

    // Now verify the files were actually created
    let config_path = dir.path().join("config.toml");
    let identity_path = dir.path().join("identity.key");

    assert!(config_path.exists(), "config.toml should have been created");
    assert!(
        identity_path.exists(),
        "identity.key should have been created"
    );

    // Bonus: verify the config file contains valid TOML with our defaults
    let config_content = fs::read_to_string(&config_path).expect("should read config");
    assert!(config_content.contains("port = 8443"));
    assert!(config_content.contains("sync_enabled = true"));
}
