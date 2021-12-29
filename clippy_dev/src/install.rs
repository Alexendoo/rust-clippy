use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, io, process};
use walkdir::WalkDir;

/// # Panics
///
/// Panics if an IO error occurs when installing the toolchain
pub fn run(name: &str) {
    let mut toolchain = PathBuf::from(env::var_os("RUSTUP_HOME").unwrap());
    toolchain.push("toolchains");
    toolchain.push(name);

    let output = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("failed to run rustc");
    assert!(output.status.success());

    let sysroot = Path::new(std::str::from_utf8(&output.stdout).unwrap().trim());

    assert_ne!(toolchain, sysroot);

    if let Err(e) = fs::remove_dir_all(&toolchain) {
        assert_eq!(
            e.kind(),
            io::ErrorKind::NotFound,
            "failed to remove {}: {:?}",
            toolchain.display(),
            e
        );
    }

    for entry in WalkDir::new(sysroot) {
        let entry = entry.unwrap();

        let src = entry.path();
        let dest = toolchain.join(src.strip_prefix(sysroot).unwrap());

        if entry.file_type().is_dir() {
            fs::create_dir(&dest).unwrap_or_else(|e| panic!("Failed to create directory {}: {:?}", dest.display(), e));
        } else {
            fs::copy(src, &dest)
                .unwrap_or_else(|e| panic!("Failed to copy {} -> {}: {:?}", src.display(), dest.display(), e));
        }
    }

    let code = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--bin",
            "cargo-clippy",
            "--bin",
            "clippy-driver",
            "-Zunstable-options",
            "--out-dir",
        ])
        .arg(toolchain.join("bin"))
        .status()
        .expect("failed to run cargo")
        .code();

    if code.is_none() {
        eprintln!("Killed by signal");
    }

    process::exit(code.unwrap_or(1));
}
