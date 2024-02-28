#![allow(rustc::diagnostic_outside_of_impl)]
#![allow(rustc::untranslatable_diagnostic)]
#![feature(rustc_private)]
#![feature(let_chains)]
#![feature(lazy_cell)]
#![feature(lint_reasons)]
#![cfg_attr(feature = "deny-warnings", deny(warnings))]
// warn on lints, that are included in `rust-lang/rust`s bootstrap
#![warn(rust_2018_idioms, unused_lifetimes)]
// warn on rustc internal lints
#![warn(rustc::internal)]

// FIXME: switch to something more ergonomic here, once available.
// (Currently there is no way to opt into sysroot crates without `extern crate`.)
extern crate rustc_driver;
extern crate rustc_session;

use rustc_session::config::ErrorOutputType;
use rustc_session::EarlyDiagCtxt;

use std::env;
use std::process::exit;

struct ClippyCallbacks;

impl rustc_driver::Callbacks for ClippyCallbacks {}

pub fn main() {
    let early_dcx = EarlyDiagCtxt::new(ErrorOutputType::default());

    rustc_driver::init_rustc_env_logger(&early_dcx);

    exit(rustc_driver::catch_with_exit_code(move || {
        let args: Vec<String> = env::args().collect();

        rustc_driver::RunCompiler::new(&args, &mut ClippyCallbacks {}).run()
    }))
}
