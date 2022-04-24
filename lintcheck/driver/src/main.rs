use regex::Regex;
use serde::Deserialize;
use std::process::{self, Command, ExitStatus};
use std::{env, fs};

#[derive(Deserialize)]
struct SourcesToml {
    #[serde(default)]
    recursive: Recursive,
}

#[derive(Deserialize, Default)]
struct Recursive {
    ignore: Vec<String>,
}

#[track_caller]
fn expect_var(name: &str) -> String {
    match env::var(name) {
        Ok(s) => s,
        Err(e) => panic!("missing env {name:?}: {e:?}"),
    }
}

fn clippy(mut args: Vec<String>) -> ExitStatus {
    // clippy-driver skips running if it finds a --cap-lint allow
    let mut iter = args.iter_mut();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--cap-lints=allow" => {
                *arg = "--cap-lints=warn".to_string();
            },
            "--cap-lints" => {
                *iter.next().unwrap() = "warn".to_string();
            },
            _ => {},
        }
    }

    Command::new(expect_var("CLIPPY_DRIVER"))
        .args(args)
        .status()
        .expect("failed to run clippy-driver")
}

fn main() {
    let sources_toml = fs::read(expect_var("SOURCES_TOML")).unwrap();
    let sources_toml: SourcesToml = toml::from_slice(&sources_toml).unwrap();

    let args = env::args().skip_while(|arg| arg != "rustc");

    let ignored = match env::var("CARGO_PKG_NAME") {
        Ok(pkg_name) => sources_toml.recursive.ignore.iter().any(|glob| {
            let pattern = format!("^{}$", glob.replace('*', ".*"));
            let re = Regex::new(&pattern).unwrap();
            re.is_match(&pkg_name)
        }),
        Err(_) => true,
    };

    let status = if ignored {
        Command::new("rustc")
            .args(args.skip(1))
            .status()
            .expect("failed to run rustc")
    } else {
        clippy(args.collect())
    };

    process::exit(status.code().expect("killed by signal"))
}
