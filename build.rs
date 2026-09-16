//! Copies `memory.x` to where the linker can find it, forwards `EXPAD_*` variables (from the
//! environment or the untracked `.env` file) to the crate, and (with the `web` feature) checks the
//! WiFi password and builds the web interface in web/ into `OUT_DIR/web` for `include_bytes!`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

const WEB_DIRECTORY: &str = "web";
const WIFI_PASSWORD_VARIABLE: &str = "EXPAD_WIFI_PASSWORD";

fn main() {
    let out_directory = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    copy_memory_layout(&out_directory);
    export_environment_variables();
    if env::var_os("CARGO_FEATURE_WEB").is_some() {
        check_wifi_password();
        build_web_interface(&out_directory.join("web"));
    }
}

fn copy_memory_layout(out_directory: &Path) {
    fs::write(out_directory.join("memory.x"), include_bytes!("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out_directory.display());
    println!("cargo:rerun-if-changed=memory.x");

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}

/// See `.env.example` for the supported variables. Variables already set in the environment win.
fn export_environment_variables() {
    if dotenvy::from_path(".env").is_ok() {
        println!("cargo:rerun-if-changed=.env");
    }
    for (key, value) in env::vars().filter(|(key, _)| key.starts_with("EXPAD_")) {
        println!("cargo:rustc-env={key}={value}");
    }
}

fn check_wifi_password() {
    println!("cargo:rerun-if-env-changed={WIFI_PASSWORD_VARIABLE}");
    let password = env::var(WIFI_PASSWORD_VARIABLE).unwrap_or_default();
    let is_valid_wpa2_password = (8..=63).contains(&password.len())
        && password
            .chars()
            .all(|character| character == ' ' || character.is_ascii_graphic());
    assert!(
        is_valid_wpa2_password,
        "{WIFI_PASSWORD_VARIABLE} must be a WPA2 password of 8-63 printable ASCII characters. \
         Set it in .env (see .env.example) or build with `--no-default-features`."
    );
}

fn build_web_interface(out_directory: &Path) {
    let web_directory = Path::new(WEB_DIRECTORY);
    for entry in fs::read_dir(web_directory).unwrap().flatten() {
        if entry.file_name() != "node_modules" {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }

    let lockfile = web_directory.join("package-lock.json");
    let installed_lockfile = web_directory.join("node_modules/.package-lock.json");
    if modified_time(&installed_lockfile) < modified_time(&lockfile) {
        run_npm(&["ci"], out_directory);
    }

    run_npm(&["run", "build"], out_directory);
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn run_npm(arguments: &[&str], out_directory: &Path) {
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let status = Command::new(npm)
        .args(arguments)
        .current_dir(WEB_DIRECTORY)
        .env("EXPAD_WEB_OUT_DIR", out_directory)
        .status()
        .expect("failed to run npm; install Node.js or build with `--no-default-features`");
    assert!(
        status.success(),
        "`npm {}` failed in {WEB_DIRECTORY}/",
        arguments.join(" ")
    );
}
