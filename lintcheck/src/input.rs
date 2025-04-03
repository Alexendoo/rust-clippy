use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self};
use std::path::Path;

use cargo_util_schemas::manifest::{InheritableDependency, PackageName};

/// List of sources to check, loaded from a .toml file
#[derive(Debug, Deserialize)]
pub struct SourceList {
    pub crates: BTreeMap<PackageName, InheritableDependency>,
    #[serde(default)]
    pub recursive: RecursiveOptions,
}

#[derive(Debug, Deserialize, Default)]
pub struct RecursiveOptions {
    pub ignore: HashSet<String>,
}

impl SourceList {
    pub fn parse(toml_path: &Path) -> Self {
        let toml_content: String =
            fs::read_to_string(toml_path).unwrap_or_else(|_| panic!("Failed to read {}", toml_path.display()));
        let list: Self =
            toml::from_str(&toml_content).unwrap_or_else(|e| panic!("Failed to parse {}: \n{e}", toml_path.display()));
        for (name, dep) in &list.crates {
            let unused = dep.unused_keys();
            assert!(unused.is_empty(), "{name} has unused keys: {unused:?}");
        }
        list
    }
}
