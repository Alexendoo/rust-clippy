#![feature(lazy_cell)]

use std::env;
use std::process::Command;

#[test]
fn check() {
    println!("{:?}", std::thread::available_parallelism());

    let current_exe_path = env::current_exe().unwrap();
    let deps_path = current_exe_path.parent().unwrap();
    let profile_path = deps_path.parent().unwrap();

    let program = profile_path.join(if cfg!(windows) {
        "clippy-driver.exe"
    } else {
        "clippy-driver"
    });

    for i in 0..5000 {
        let mut c = Command::new(&program);
        c.arg("tests/ui/hello_world.rs");
        c.arg("--out-dir=target/ui");
        let out = c.output().unwrap();
        if !out.status.success() {
            println!("run {i}");
            println!("{c:?}");
            println!("status: {}", out.status);
            println!("stdout:\n{}", std::str::from_utf8(&out.stdout).unwrap());
            println!("stderr:\n{}", std::str::from_utf8(&out.stderr).unwrap());

            panic!();
        }
    }
}
