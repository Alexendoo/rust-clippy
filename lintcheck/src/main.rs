// Run clippy on a fixed set of crates and collect the warnings.
// This helps observing the impact clippy changes have on a set of real-world code (and not just our
// testsuite).
//
// When a new lint is introduced, we can search the results for new warnings and check for false
// positives.

#![feature(let_chains)]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    rust_2018_idioms,
    unused_lifetimes,
    unused_qualifications
)]
#![allow(
    clippy::collapsible_else_if,
    clippy::needless_borrows_for_generic_args,
    clippy::module_name_repetitions,
    clippy::literal_string_with_formatting_args
)]

mod config;
mod driver;
mod input;
mod json;
mod output;
mod popular_crates;
mod recursive;

use crate::config::{Commands, LintcheckConfig, OutputFormat};
use crate::input::SourceList;
use crate::recursive::LintcheckServer;

use std::cmp::max_by_key;
use std::collections::BTreeMap;
use std::env::consts::EXE_SUFFIX;
use std::fs::File;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs, io};

use cargo_metadata::{Metadata, MetadataCommand, Package};
use cargo_util_schemas::manifest::{
    InheritableField, PackageName, TomlDependency, TomlDetailedDependency, TomlManifest, TomlPackage, TomlWorkspace,
};
use input::RecursiveOptions;
use walkdir::WalkDir;

fn run_clippy_lints(clippy_driver_path: &Path, config: &LintcheckConfig, metadata: &Metadata, server_addr: SocketAddr) {
    let mut clippy_args: Vec<String> = if config.lint_filter.is_empty() {
        let groups = if config.all_lints {
            &[
                "clippy::all",
                "clippy::cargo",
                "clippy::nursery",
                "clippy::pedantic",
                "clippy::restriction",
            ][..]
        } else {
            &["clippy::all", "clippy::pedantic"]
        };
        groups.iter().map(|group| format!("--force-warn={group}")).collect()
    } else {
        config
            .lint_filter
            .iter()
            .map(|filter| format!("--force-warn={filter}"))
            .collect()
    };

    // Add `target/lintcheck` to the front of `sources/crate-1.2.3/src/lib.rs`
    clippy_args.push("--remap-path-prefix==target/lintcheck".into());

    let mut cmd = Command::new("cargo");

    if config.perf {
        cmd.env("PERF_DIR", get_perf_dir());
    }

    // `cargo clippy` is a wrapper around `cargo check` that mainly sets `RUSTC_WORKSPACE_WRAPPER` to
    // `clippy-driver`. We do the same thing here except the wrapper is set to `lintcheck` itself so we
    // can ignore duplicates and run `perf` if requested (see `crate::driver`)
    cmd.arg("check")
        .arg("-Zavoid-dev-deps")
        .arg("--target-dir=build")
        .current_dir("target/lintcheck")
        .env("CLIPPY_DISABLE_DOCS_LINKS", "1")
        // Hide rustc lints, clippy lints bypass this with --force-warn
        .env("RUSTFLAGS", "--cap-lints=allow")
        .env("RUSTC_WORKSPACE_WRAPPER", env::current_exe().unwrap())
        // Pass the absolute path so `crate::driver` can find `clippy-driver`, as it's executed in various
        // different working directories
        .env("CLIPPY_DRIVER", clippy_driver_path)
        .env("LINTCHECK_SERVER", server_addr.to_string());

    if let Some(only) = config.only.as_deref() {
        let package = specified_packages(metadata)
            .into_iter()
            .find(|pkg| pkg.name == only)
            .unwrap();
        cmd.arg(format!("-p{}@{}", package.name, package.version));
        clippy_args.push("--no-deps".into());
    }

    // If recursive is enabled specify the base set of crates with `-p`, this keeps the unified set of
    // enabled features the same as a regular build to avoid building extra dependencies
    if config.recursive {
        for package in specified_packages(metadata) {
            cmd.arg(format!("-p{}@{}", package.name, package.version));
        }
    }

    let status = cmd
        .env("CLIPPY_ARGS", clippy_args.join("__CLIPPY_HACKERY__"))
        .status()
        .expect("failed to run cargo");

    assert_eq!(status.code(), Some(0));
}

/// Builds clippy inside the repo to make sure we have a clippy executable we can use.
fn build_clippy(release_build: bool) -> String {
    let mut build_cmd = Command::new("cargo");
    build_cmd.args(["run", "--bin=clippy-driver"]);
    if release_build {
        build_cmd.arg("--release");
    }

    if release_build {
        build_cmd.env("CARGO_PROFILE_RELEASE_DEBUG", "true");
    }

    let output = build_cmd
        .args(["--", "--version"])
        .stderr(Stdio::inherit())
        .output()
        .unwrap();

    if !output.status.success() {
        eprintln!("Error: Failed to compile Clippy!");
        std::process::exit(1);
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn main() {
    // We're being executed as a `RUSTC_WRAPPER`
    if let Ok(addr) = env::var("LINTCHECK_SERVER") {
        driver::drive(&addr);
    }

    // assert that we launch lintcheck from the repo root (via cargo lintcheck)
    if fs::metadata("lintcheck/Cargo.toml").is_err() {
        eprintln!("lintcheck needs to be run from clippy's repo root!\nUse `cargo lintcheck` alternatively.");
        std::process::exit(3);
    }

    let config = LintcheckConfig::new();

    match config.subcommand {
        Some(Commands::Diff { old, new, truncate }) => json::diff(&old, &new, truncate),
        Some(Commands::Popular { output, number }) => popular_crates::fetch(output, number).unwrap(),
        None => lintcheck(config),
    }
}

fn lintcheck(config: LintcheckConfig) {
    let clippy_ver = build_clippy(config.perf);
    let clippy_driver_path = fs::canonicalize(format!(
        "target/{}/clippy-driver{EXE_SUFFIX}",
        if config.perf { "release" } else { "debug" }
    ))
    .unwrap();
    assert!(clippy_driver_path.is_file());

    ok_or_not_found(fs::remove_dir_all("target/lintcheck/build"));

    let sources = SourceList::parse(&config.sources_toml_path);
    let metadata = resolve_crates(&sources, &config);

    let packages = if config.recursive {
        recursive_packages(&metadata, sources.recursive)
    } else {
        specified_packages(&metadata)
    };

    create_workspace(&packages);

    let server = LintcheckServer::spawn(config.perf);

    run_clippy_lints(&clippy_driver_path, &config, &metadata, server.local_addr);

    let warnings = server.warnings();

    let text = match config.format {
        OutputFormat::Text | OutputFormat::Markdown => {
            output::summarize_and_print_changes(&warnings, clippy_ver, &config)
        },
        OutputFormat::Json => json::output(warnings),
    };

    println!("Writing logs to {}", config.lintcheck_results_path.display());
    fs::create_dir_all(config.lintcheck_results_path.parent().unwrap()).unwrap();
    fs::write(&config.lintcheck_results_path, text).unwrap();
}

fn resolve_crates(sources: &SourceList, config: &LintcheckConfig) -> Metadata {
    fs::create_dir_all("target/lintcheck/src").unwrap();
    File::create("target/lintcheck/src/lib.rs").unwrap();

    let lock_path = config.sources_toml_path.with_extension("lock");
    // Remove a Cargo.lock from a previous run in case we don't have an existing one to copy over
    ok_or_not_found(fs::remove_file("target/lintcheck/Cargo.lock"));
    ok_or_not_found(fs::copy(&lock_path, "target/lintcheck/Cargo.lock"));

    let mut package = TomlPackage::new(PackageName::new("lintcheck_sources".into()).unwrap());
    package.edition = Some(InheritableField::Value("2024".into()));

    let mut manifest = TomlManifest::default();
    manifest.package = Some(Box::new(package));
    manifest.dependencies = Some(sources.crates.clone());

    fs::write(
        "target/lintcheck/Cargo.toml",
        toml::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let metadata = MetadataCommand::new()
        .manifest_path("target/lintcheck/Cargo.toml")
        .verbose(true)
        .other_options(
            [
                "-Zbindeps",
                "--filter-platform",
                env!("TARGET"),
                // Use the versions listed in the sources toml
                "-Zdirect-minimal-versions",
            ]
            .map(str::to_string),
        )
        .exec()
        .unwrap();

    fs::copy("target/lintcheck/Cargo.lock", &lock_path).unwrap();

    metadata
}

fn specified_packages<'a>(metadata: &'a Metadata) -> Vec<&'a Package> {
    let resolve = metadata.resolve.as_ref().unwrap();
    let root = resolve.root.as_ref().unwrap();
    resolve[root].deps.iter().map(|dep| &metadata[&dep.pkg]).collect()
}

fn recursive_packages<'a>(metadata: &'a Metadata, options: RecursiveOptions) -> Vec<&'a Package> {
    let mut packages = BTreeMap::<&str, &Package>::new();
    for package in &metadata.packages {
        if options.ignore.contains(&package.name) {
            continue;
        }

        packages
            .entry(&package.name)
            .and_modify(|prev| *prev = max_by_key(prev, package, |pkg| &pkg.version))
            .or_insert(package);
    }
    for package in specified_packages(metadata) {
        packages.insert(&package.name, package);
    }
    packages.remove(metadata.root_package().unwrap().name.as_str());
    packages.into_values().collect()
}

fn create_workspace(packages: &[&Package]) {
    fs::create_dir_all("target/lintcheck/sources").unwrap();

    let mut members = Vec::new();
    let mut patch = BTreeMap::new();
    for package in packages {
        let workspace_path = format!("sources/{}-{}", package.name, package.version);
        let package_dir = Path::new("target/lintcheck").join(&workspace_path);

        copy_to_crates_dir(package, &package_dir);
        hide_cargo_warnings(&package_dir);

        members.push(workspace_path.clone());
        patch.insert(
            PackageName::new(package.name.clone()).unwrap(),
            TomlDependency::Detailed(TomlDetailedDependency {
                path: Some(workspace_path),
                ..TomlDetailedDependency::default()
            }),
        );
    }

    let workspace = TomlWorkspace {
        members: Some(members),
        resolver: Some("2".into()),
        ..TomlWorkspace::default()
    };

    let manifest = TomlManifest {
        workspace: Some(workspace),
        patch: Some(BTreeMap::from([("crates-io".into(), patch)])),
        ..TomlManifest::default()
    };

    fs::write(
        "target/lintcheck/Cargo.toml",
        toml::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

/// Copy e.g. `~/.cargo/registry/src/index.crates.io-6f17d22bba15001f/ripgrep-14.1.0/..` to
/// `target/lintcheck/sources/ripgrep-14.1.0/..`
fn copy_to_crates_dir(package: &Package, package_dir: &Path) {
    let registry_dir = package.manifest_path.parent().unwrap();

    if package_dir.exists() {
        return;
    }

    for entry in WalkDir::new(registry_dir) {
        let entry = entry.unwrap();
        let relative = entry.path().strip_prefix(registry_dir).unwrap();
        let target = package_dir.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir(target).unwrap();
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Hide some Cargo warnings, in the future this could be replaced by <https://github.com/rust-lang/cargo/issues/12235>
fn hide_cargo_warnings(package_dir: &Path) {
    let path = package_dir.join("Cargo.toml");
    let src = fs::read_to_string(&path).unwrap();
    let mut manifest: TomlManifest = toml::from_str(&src).unwrap();

    manifest.profile = None;
    if let Some(package) = manifest.package.as_deref_mut() {
        package.resolver = None;
        if package.edition.is_none() {
            package.edition = Some(InheritableField::Value("2015".into()));
        }
    }

    let updated = toml::to_string_pretty(&manifest).unwrap();
    fs::write(&path, updated).unwrap();
}

fn get_perf_dir() -> PathBuf {
    let base = Path::new("target/lintcheck/perf");

    let run_number = if fs::create_dir(base).is_ok() {
        0
    } else {
        base.read_dir()
            .unwrap()
            .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
            .max()
            .map_or(0, |max| max + 1)
    };

    let new_dir = base.canonicalize().unwrap().join(run_number.to_string());
    fs::create_dir(&new_dir).unwrap();
    new_dir
}

fn ok_or_not_found<T>(result: io::Result<T>) {
    if let Err(e) = result {
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
    }
}
