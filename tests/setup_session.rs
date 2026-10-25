use std::process::Command;

#[test]
fn ssh_without_a_desktop_fails_before_writing_configuration() {
    let root = std::env::temp_dir().join(format!("wayshare-no-session-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wayshare"))
        .arg("setup")
        .env("SSH_CONNECTION", "192.0.2.1 1234 192.0.2.2 22")
        .env("XDG_RUNTIME_DIR", root.join("runtime"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(message.contains("Hyprland session"), "{message}");
    assert!(!root.join("config").exists());
    assert!(!root.join("data").exists());
    std::fs::remove_dir_all(root).unwrap();
}
