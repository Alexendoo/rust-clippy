// compile-flags: -Z deduplicate-diagnostics=yes
#![feature(custom_inner_attributes)]
#![clippy::msrv = "invalid.version"]

fn main() {}
