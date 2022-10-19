// compile-flags: -Z deduplicate-diagnostics=yes
// run-rustfix

#[clippy::cyclomatic_complexity = "1"]
fn main() {}
