#![feature(lazy_cell)]

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::process::Command;
use std::thread;
use walkdir::WalkDir;

#[test]
fn test() {
    thread::scope(|s| {
        for t in 0..thread::available_parallelism().unwrap().get() {
            s.spawn(move || check(t));
        }
    })
}

fn check(thread: usize) {
    let mut args = Vec::new();
    args.extend(
        [
            "--emit=metadata",
            "-Aunused",
            "-Ainternal_features",
            "-Zui-testing",
            "-Dwarnings",
        ]
        .map(OsString::from),
    );

    for f in WalkDir::new("tests/ui") {
        let entry = f.unwrap();
        if entry.path().extension() == Some(OsStr::new("rs")) {
            let mut c = Command::new("rustc");
            c.args(&args);
            c.arg(entry.path());
            c.arg(format!("--out-dir=target/ui-{thread}"));
            let out = c.output().unwrap();
            if out.status.code().is_none() {
                let mut o = io::stdout().lock();
                writeln!(o, "thread {thread}").unwrap();
                writeln!(o, "{c:?}").unwrap();
                writeln!(o, "status: {}", out.status).unwrap();
                writeln!(o, "stdout:\n{}", std::str::from_utf8(&out.stdout).unwrap()).unwrap();
                writeln!(o, "stderr:\n{}", std::str::from_utf8(&out.stderr).unwrap()).unwrap();

                panic!();
            }
        }
    }
}
