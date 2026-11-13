use super::*;

#[test]
fn nightly_builds_and_dated_toolchains_are_distinct() {
    assert!(!same_rust_build(
        "rustc 1.101.0-nightly (1111111111 2026-11-12)",
        "1.101.0-nightly (2222222222 2026-11-13)"
    ));
    assert!(same_rust_build(
        "rustc 1.99.1 (abcdef1234 2026-11-12)",
        "1.99.1 (abcdef1 2026-11-12)"
    ));
    assert!(!same_rust_build("1.101.0-nightly", "1.101.0-nightly"));
    assert_eq!(
        channel_from_toolchain("nightly-2026-11-01-x86_64-pc-windows-msvc"),
        "pinned"
    );
    assert_eq!(
        channel_from_toolchain("nightly-x86_64-pc-windows-msvc"),
        "nightly"
    );
}

#[test]
fn cancelled_commands_do_not_start_a_process() {
    let id = "cancelled-command-regression";
    let control = background::register(id, "toolchain");
    control.cancel();
    let result = background::with_task(id, || {
        run_streaming("rustc", &["--version"], |_| {
            panic!("cancelled command produced output")
        })
    });
    background::unregister(id);
    assert!(result.unwrap_err().contains("任务已取消"));
}

#[test]
fn toolchain_mutations_are_serialized() {
    let guard = begin_mutation().unwrap();
    assert!(begin_mutation().is_err());
    drop(guard);
    assert!(begin_mutation().is_ok());
}

#[test]
fn toolchain_operations_use_isolated_mock() {
    if let Some(home) = env::var_os("CARGO_SWEEP_MOCK_DIR") {
        exercise_mock_toolchain_operations(Path::new(&home));
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("cargo/bin");
    fs::create_dir_all(&bin).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_rustup.rs");
    let output = Command::new("rustc")
        .args(["--edition=2021", "--crate-name", "mock_rustup"])
        .env("TEMP", temp.path())
        .env("TMP", temp.path())
        .env("TMPDIR", temp.path())
        .arg(fixture)
        .arg("-o")
        .arg(bin.join(binary_name("rustup")))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(env::current_exe().unwrap())
        .args([
            "--exact",
            "rust_toolchain::operation_tests::toolchain_operations_use_isolated_mock",
            "--nocapture",
        ])
        .env("CARGO_SWEEP_MOCK_DIR", temp.path())
        .env("CARGO_HOME", temp.path().join("cargo"))
        .env("RUSTUP_HOME", temp.path().join("rustup"))
        .env("LOCALAPPDATA", temp.path().join("local"))
        .env("APPDATA", temp.path().join("config"))
        .env("XDG_DATA_HOME", temp.path().join("local"))
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("HOME", temp.path())
        .env("RUSTUP_DIST_SERVER", "https://mock.invalid")
        .env("RUSTUP_UPDATE_ROOT", "https://mock.invalid/rustup")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "mock test stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn exercise_mock_toolchain_operations(home: &Path) {
    const STABLE: &str = "stable-x86_64-pc-windows-msvc";
    const NIGHTLY: &str = "nightly-x86_64-pc-windows-msvc";
    const NEW: &str = "1.99.1 (2222222222 2026-11-12)";
    fs::write(
        home.join("state"),
        format!("{STABLE}|1.99.0 (1111111111 2026-11-01)\n"),
    )
    .unwrap();
    fs::write(home.join("default"), STABLE).unwrap();
    let behavior = |value| fs::write(home.join("behavior"), value).unwrap();
    // A successful exit with an unchanged old compiler is NOT success.
    behavior("update-noop");
    let result = update_with_progress(Some(STABLE.into()), Some(NEW.into()), |_| {}).unwrap();
    assert!(!result.success);
    assert!(result.output.contains("更新校验失败"));
    assert_eq!(result.status.version.as_deref(), Some("1.99.0"));
    // Conflicts must preserve the old toolchain, not implicitly uninstall it.
    behavior("update-conflict");
    let result = update_with_progress(Some(STABLE.into()), Some(NEW.into()), |_| {}).unwrap();
    assert!(!result.success);
    assert_eq!(result.status.default_toolchain.as_deref(), Some(STABLE));
    assert!(!fs::read_to_string(home.join("commands"))
        .unwrap()
        .contains("toolchain uninstall"));
    // A stale mirror is retried against the official source, scoped to the child command.
    behavior("mirror-stale");
    let result = update_with_progress(Some(STABLE.into()), Some(NEW.into()), |_| {}).unwrap();
    assert!(result.success, "{}", result.output);
    assert!(result.output.contains("改用官方源重试"));
    assert_eq!(
        env::var("RUSTUP_DIST_SERVER").unwrap(),
        "https://mock.invalid"
    );
    // Retry and an already-current build both succeed without destructive repair.
    behavior("");
    for _ in 0..2 {
        let result = update_with_progress(Some(STABLE.into()), Some(NEW.into()), |_| {}).unwrap();
        assert!(result.success, "{}", result.output);
        assert_eq!(result.status.version.as_deref(), Some("1.99.1"));
    }
    behavior("install-noop");
    assert!(
        !install_with_progress("nightly".into(), false, |_| {})
            .unwrap()
            .success
    );
    behavior("");
    assert!(
        install_with_progress("nightly".into(), false, |_| {})
            .unwrap()
            .success
    );
    assert!(uninstall(STABLE.into()).is_err());
    behavior("delete-noop");
    let result = uninstall(NIGHTLY.into()).unwrap();
    assert!(!result.success);
    assert!(result
        .status
        .toolchains
        .iter()
        .any(|toolchain| toolchain.name == NIGHTLY));
    // Cancel an in-flight uninstall; only the mock child process is terminated.
    behavior("delete-slow");
    let id = "mock-cancel-uninstall";
    let control = background::register(id, "toolchain");
    let task = thread::spawn(move || background::with_task(id, || uninstall(NIGHTLY.into())));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !home.join("deleting").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    let started = home.join("deleting").exists();
    control.cancel();
    let result = task.join().unwrap();
    background::unregister(id);
    assert!(started, "mock uninstall never started");
    let result = result.unwrap();
    assert!(!result.success);
    assert!(result.output.contains("任务已取消"));
    assert!(result
        .status
        .toolchains
        .iter()
        .any(|toolchain| toolchain.name == NIGHTLY));
    behavior("");
    let result = uninstall(NIGHTLY.into()).unwrap();
    assert!(result.success);
    assert!(!result
        .status
        .toolchains
        .iter()
        .any(|toolchain| toolchain.name == NIGHTLY));
    assert!(
        install_with_progress("nightly".into(), false, |_| {})
            .unwrap()
            .success
    );
    behavior("delete-list-failure");
    assert!(uninstall(NIGHTLY.into()).unwrap_err().contains("无法确认"));
    fs::remove_file(home.join("fail-list")).unwrap();
    // A failed post-operation query cannot silently become stale-data success.
    behavior("update-list-failure");
    assert!(
        update_with_progress(Some(STABLE.into()), Some(NEW.into()), |_| {})
            .unwrap_err()
            .contains("无法确认")
    );
    fs::remove_file(home.join("fail-list")).unwrap();
    fs::write(home.join("state"), "").unwrap();
    fs::write(home.join("default"), "").unwrap();
    assert!(detect().unwrap().toolchains.is_empty());
}

#[test]
fn rolling_channels_ignore_pinned_builds_and_prefer_the_current_host() {
    let make = |name: &str| InstalledToolchain {
        name: name.into(),
        channel: channel_from_toolchain(name),
        release: Some("1.101.0-nightly".into()),
        rustc_version: Some("rustc 1.101.0-nightly (3333333333 2026-11-13)".into()),
        active: false,
        default: false,
    };
    let toolchains = vec![
        make("nightly-2026-11-01-x86_64-pc-windows-msvc"),
        make("nightly-aarch64-apple-darwin"),
        make("nightly-x86_64-pc-windows-msvc"),
    ];
    assert_eq!(
        channel_toolchain(&toolchains, "nightly", Some("x86_64-pc-windows-msvc"))
            .unwrap()
            .name,
        "nightly-x86_64-pc-windows-msvc"
    );
    assert!(channel_toolchain(&toolchains[..1], "nightly", None).is_none());
}
