//! install.sh: target detection, checksum verification and install location,
//! driven offline through `WTEAM_DOWNLOAD_BASE=file://...`.

use std::path::Path;
use std::process::Command;

const TARGETS: &[(&str, &str, &str)] = &[
    ("Linux", "x86_64", "x86_64-unknown-linux-musl"),
    ("Linux", "aarch64", "aarch64-unknown-linux-musl"),
    ("Darwin", "arm64", "aarch64-apple-darwin"),
    ("Darwin", "x86_64", "x86_64-apple-darwin"),
];

fn sha256(path: &Path) -> String {
    let out = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

/// A fake release dir holding one script per target plus checksums.txt.
fn release(dir: &Path, corrupt: Option<&str>) {
    let mut sums = String::new();
    for (_, _, target) in TARGETS {
        let p = dir.join(format!("wteam-{target}"));
        std::fs::write(&p, format!("#!/bin/sh\necho {target}\n")).unwrap();
        let sum = if corrupt == Some(*target) {
            "0".repeat(64)
        } else {
            sha256(&p)
        };
        sums.push_str(&format!("{sum}  wteam-{target}\n"));
    }
    std::fs::write(dir.join("checksums.txt"), sums).unwrap();
}

fn install(dir: &Path, bin: &Path, os: &str, arch: &str) -> std::process::Output {
    Command::new("sh")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh"))
        .env("WTEAM_DOWNLOAD_BASE", format!("file://{}", dir.display()))
        .env("WTEAM_INSTALL_DIR", bin)
        .env("WTEAM_OS", os)
        .env("WTEAM_ARCH", arch)
        .output()
        .expect("run install.sh")
}

#[test]
fn installs_the_matching_target_for_each_platform() {
    let rel = tempfile::tempdir().unwrap();
    release(rel.path(), None);
    for (os, arch, target) in TARGETS {
        let bin = tempfile::tempdir().unwrap();
        let out = install(rel.path(), bin.path(), os, arch);
        assert!(
            out.status.success(),
            "{os}/{arch}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let ran = Command::new(bin.path().join("wteam")).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), *target);
    }
}

#[test]
fn checksum_mismatch_refuses_to_install() {
    let rel = tempfile::tempdir().unwrap();
    release(rel.path(), Some("x86_64-unknown-linux-musl"));
    let bin = tempfile::tempdir().unwrap();
    let out = install(rel.path(), bin.path(), "Linux", "x86_64");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("checksum mismatch"));
    assert!(!bin.path().join("wteam").exists());
}

#[test]
fn unsupported_platform_points_at_a_source_build() {
    let rel = tempfile::tempdir().unwrap();
    release(rel.path(), None);
    let bin = tempfile::tempdir().unwrap();
    let out = install(rel.path(), bin.path(), "FreeBSD", "x86_64");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("cargo install --git"));
}
