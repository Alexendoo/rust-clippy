#![feature(lazy_cell)]

use std::process::Command;
use std::thread;

#[test]
fn check() {
    println!("{:?}", thread::available_parallelism());

    thread::scope(|s| {
        for t in 0..thread::available_parallelism().unwrap().get() {
            s.spawn(move || {
                for i in 0..5000 {
                    let mut c = Command::new("rustc");
                    c.arg("tests/ui/hello_world.rs");
                    c.arg(format!("--out-dir=target/ui-{t}"));
                    let out = c.output().unwrap();
                    if !out.status.success() {
                        println!("run {i}, thread {t}");
                        println!("{c:?}");
                        println!("status: {}", out.status);
                        println!("stdout:\n{}", std::str::from_utf8(&out.stdout).unwrap());
                        println!("stderr:\n{}", std::str::from_utf8(&out.stderr).unwrap());

                        panic!();
                    }
                }
            });
        }
    });
}
