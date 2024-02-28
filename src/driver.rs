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

const BUG_REPORT_URL: &str = "https://github.com/rust-lang/rust-clippy/issues/new?template=ice.yml";

#[allow(clippy::too_many_lines)]
#[allow(clippy::ignored_unit_patterns)]
pub fn main() {
    let early_dcx = EarlyDiagCtxt::new(ErrorOutputType::default());

    rustc_driver::init_rustc_env_logger(&early_dcx);

    let using_internal_features = rustc_driver::install_ice_hook(BUG_REPORT_URL, |handler| {
        // FIXME: this macro calls unwrap internally but is called in a panicking context!  It's not
        // as simple as moving the call from the hook to main, because `install_ice_hook` doesn't
        // accept a generic closure.
        let version_info = rustc_tools_util::get_version_info!();
        handler.note(format!("Clippy version: {version_info}"));
    });

    exit(rustc_driver::catch_with_exit_code(move || {
        let args: Vec<String> = env::args().collect();

        rustc_driver::RunCompiler::new(&args, &mut ClippyCallbacks {})
            .set_using_internal_features(using_internal_features)
            .run()
    }))
}
