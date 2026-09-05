use std::process::Command;

fn driver() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-cosmopolitan"));
    command.env_remove("CARGO_COSMO_LINK_CC");
    command
}

#[test]
fn cargo_subcommand_help_needs_no_toolchain() {
    let output = driver()
        .args(["cosmopolitan", "--help"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("preflight|build|run"));
}

#[test]
fn preflight_reports_missing_cosmopolitan() {
    let output = driver().arg("preflight").env("PATH", "").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cosmocc was not found on PATH"));
}

#[test]
fn conflicting_cargo_target_is_rejected_before_building() {
    let output = driver()
        .args(["build", "--target", "x86_64-unknown-linux-gnu"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported option --target"));
}

#[test]
fn program_help_stays_with_the_program() {
    let output = driver()
        .args(["run", "--", "--help"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cosmocc was not found on PATH"));
}

#[test]
fn linker_preserves_arguments_and_defers_symbol_stripping() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let directory = std::env::temp_dir().join(format!(
        "cargo-cosmopolitan-link-test-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let compiler = directory.join("fake compiler");
    fs::write(&compiler, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    let output = driver()
        .env("CARGO_COSMO_LINK_CC", &compiler)
        .args([
            "a path with spaces.o",
            "-lc",
            "-lpthread",
            "-Wl,--strip-all",
            "-L",
            "a library path",
            "-o",
            "output",
        ])
        .output()
        .unwrap();
    fs::remove_dir_all(&directory).unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "a path with spaces.o\n-L\na library path\n-o\noutput\n"
    );
}
