// Offline rustup stand-in. Only used by a child test with isolated homes.
use std::{env, fs, io::Write, path::Path, process};

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    process::exit(1)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn main() {
    let home = env::var_os("CARGO_SWEEP_MOCK_DIR").expect("isolated mock directory");
    let home = Path::new(&home);
    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("commands"))
        .unwrap();
    writeln!(log, "{}", args.join(" ")).unwrap();
    args.retain(|arg| arg != "-v");
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let behavior = read(&home.join("behavior"));
    let state_path = home.join("state");
    let mut state: Vec<(String, String)> = read(&state_path)
        .lines()
        .filter_map(|line| {
            line.split_once('|')
                .map(|(name, version)| (name.into(), version.into()))
        })
        .collect();
    let default = read(&home.join("default"));
    let host = "x86_64-pc-windows-msvc";
    let canonical = |name: &str| {
        if matches!(name, "stable" | "nightly" | "beta") {
            format!("{name}-{host}")
        } else {
            name.to_string()
        }
    };
    match args.as_slice() {
        ["--version"] => println!("rustup 1.28.0 (offline-test)"),
        ["show", "active-toolchain"] | ["default"] => {
            if default.is_empty() {
                fail("no default toolchain configured")
            }
            println!("{default} (default)");
        }
        ["default", name] => {
            fs::write(home.join("default"), canonical(name)).unwrap();
        }
        ["toolchain", "list"] => {
            if home.join("fail-list").exists() {
                fail("mock toolchain list failed")
            }
            if state.is_empty() {
                println!("no installed toolchains");
            }
            for (name, _) in &state {
                println!(
                    "{name}{}",
                    if name == &default {
                        " (active, default)"
                    } else {
                        ""
                    }
                );
            }
        }
        ["run", name, "rustc", ..] => {
            let name = canonical(name);
            let (_, version) = state
                .iter()
                .find(|(installed, _)| installed == &name)
                .unwrap_or_else(|| fail("toolchain is not installed"));
            println!(
                "rustc {version}\nrelease: {}\nhost: {host}",
                version.split_whitespace().next().unwrap()
            );
        }
        ["run", _, "cargo", "--version"] => println!("cargo 1.99.1 (offline-test)"),
        ["update", ..] => {
            if behavior == "update-conflict" {
                fail("could not rename component: directory not empty")
            }
            if behavior != "update-noop"
                && !(behavior == "mirror-stale" && env::var_os("RUSTUP_DIST_SERVER").is_some())
            {
                let target = canonical(args.last().unwrap());
                let (_, version) = state
                    .iter_mut()
                    .find(|(name, _)| name == &target)
                    .unwrap_or_else(|| fail("unknown update target"));
                *version = "1.99.1 (2222222222 2026-11-12)".into();
                fs::write(
                    &state_path,
                    state
                        .iter()
                        .map(|(name, version)| format!("{name}|{version}\n"))
                        .collect::<String>(),
                )
                .unwrap();
            }
            if behavior == "update-list-failure" {
                fs::write(home.join("fail-list"), "").unwrap();
            }
            println!("info: update command completed");
        }
        ["toolchain", "install", name, ..] => {
            if behavior != "install-noop" {
                let name = canonical(name);
                if !state.iter().any(|(installed, _)| installed == &name) {
                    state.push((name, "1.101.0-nightly (3333333333 2026-11-13)".into()));
                }
                fs::write(
                    &state_path,
                    state
                        .iter()
                        .map(|(name, version)| format!("{name}|{version}\n"))
                        .collect::<String>(),
                )
                .unwrap();
            }
            println!("info: install command completed");
        }
        ["toolchain", "uninstall", name] => {
            if behavior == "delete-slow" {
                fs::write(home.join("deleting"), "").unwrap();
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            if behavior != "delete-noop" {
                state.retain(|(installed, _)| installed != name);
                fs::write(
                    &state_path,
                    state
                        .iter()
                        .map(|(name, version)| format!("{name}|{version}\n"))
                        .collect::<String>(),
                )
                .unwrap();
            }
            if behavior == "delete-list-failure" {
                fs::write(home.join("fail-list"), "").unwrap();
            }
            println!("info: uninstall command completed");
        }
        _ => fail(&format!("unexpected mock command: {args:?}")),
    }
}
