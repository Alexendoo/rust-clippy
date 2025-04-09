use bstr::ByteSlice;
use bstr::io::BufReadExt;
use serde::{Deserialize, Serialize};

use crate::recursive::{DriverInfo, deserialize_line, serialize_line};

use std::env;
use std::fmt::Write as _;
use std::io::{self, BufReader, Write};
use std::net::TcpStream;
use std::process::{self, Command, Stdio};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub(crate) enum DriverMode {
    Clippy,
    Perf,
    Rustc,
}

fn perf_data_path(package: &DriverInfo) -> String {
    let mut path = env::var("PERF_DIR").unwrap();
    let _ = write!(&mut path, "/{}", package.package_name);
    let crate_name = env::var("CARGO_CRATE_NAME").unwrap();
    if crate_name != package.package_name {
        let _ = write!(&mut path, ".{crate_name}");
    }
    path.push_str(".data");
    path
}

/// 1. Sends [`PackageInfo`] to the [`crate::recursive::LintcheckServer`] running on `addr`
/// 2. Receives [`DriverMode`] from the server and acts acordingly
fn run_clippy(addr: &str) -> Option<i32> {
    let info = DriverInfo {
        package_name: env::var("CARGO_PKG_NAME").ok()?,
        crate_name: env::var("CARGO_CRATE_NAME").ok()?,
        version: env::var("CARGO_PKG_VERSION").ok()?,
    };

    let mut stream = BufReader::new(TcpStream::connect(addr).unwrap());

    serialize_line(&info, stream.get_mut());

    let clippy_driver = || env::var("CLIPPY_DRIVER").unwrap();
    let mut cmd;
    match deserialize_line::<DriverMode, _>(&mut stream) {
        DriverMode::Clippy => cmd = Command::new(clippy_driver()),
        DriverMode::Perf => {
            cmd = Command::new("perf");
            cmd.args([
                "record",
                "-e",
                "instructions", // Only count instructions
                "-g",           // Enable call-graph, useful for flamegraphs and produces richer reports
                "--quiet",      // Do not tamper with clippy's normal output
                "--compression-level=22",
                "--freq=3000",
                "-o",
                &perf_data_path(&info),
                "--",
                &clippy_driver(),
            ]);
        },
        DriverMode::Rustc => return None,
    }

    let mut child = cmd
        .args(env::args_os().skip(1))
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run clippy-driver");

    let mut child_stderr = BufReader::new(child.stderr.take().unwrap());
    let mut diagnostics = Vec::new();
    child_stderr
        .for_byte_line_with_terminator(|line| {
            if line.contains_str(r#""$message_type":"diagnostic""#) {
                diagnostics.extend(line);
            } else {
                io::stderr().write_all(&line).unwrap();
            }
            Ok(true)
        })
        .unwrap();

    stream
        .get_mut()
        .write_all(&diagnostics)
        .unwrap_or_else(|e| panic!("{e:?} in {info:?}"));

    match child.wait().unwrap().code() {
        Some(0) => Some(0),
        code => {
            io::stderr().write_all(&diagnostics).unwrap();
            Some(code.expect("killed by signal"))
        },
    }
}

pub fn drive(addr: &str) {
    process::exit(run_clippy(addr).unwrap_or_else(|| {
        Command::new("rustc")
            .args(env::args_os().skip(2))
            .status()
            .unwrap()
            .code()
            .unwrap()
    }))
}
