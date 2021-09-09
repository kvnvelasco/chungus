use crate::module::{Location, Module};
use std::collections::{HashMap, HashSet};

// A cache of locations of packages
// and their package definitions / import definitions

#[derive(Debug, Clone)]
pub struct ModuleCache {
    cache: HashMap<Location, Module>,
}
