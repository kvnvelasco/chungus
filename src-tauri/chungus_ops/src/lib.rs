use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use parking_lot::RwLock;

use crate::error::CoreError;
use crate::file::{process_javascript_file, process_package_json};
use crate::module::{Asset, Location, Module};
use crate::resolve::Resolver;

pub mod analysis;
pub mod dependency_graph;
pub mod error;
pub mod file;
pub mod module;
pub mod module_cache;
pub mod parser;
pub mod resolve;
pub mod webpack_report;

pub fn start_resolve_project(
  project_root: impl AsRef<Path>,
  included_directories: Vec<PathBuf>,
) -> Result<Resolver, CoreError> {
  let location = Location::new(&project_root)?;
  Ok(Resolver::new(&location, included_directories))
}

#[tracing::instrument(skip(cache, target, resolver))]
pub fn build_dependency_cache(
  resolver: &Resolver,
  target: impl AsRef<Path>,
  cache: &mut HashMap<Location, Module>,
) -> Result<(), CoreError> {
  let file = Location::new(target)?;

  tracing::info!("Start build dependency cache {:?}", &file);

  let root_module = process_javascript_file(&resolver, &file)?;
  cache.insert(file.clone(), root_module.clone());
  recursively_build_dependency_tree(cache, &resolver, root_module)?;

  tracing::info!("Built dependency cache {:?}", &file);
  Ok(())
}

pub type DependencyCache = HashMap<Location, Module>;

#[tracing::instrument(skip(cache, resolver, module))]
pub fn recursively_build_dependency_tree(
  cache: &mut DependencyCache,
  resolver: &Resolver,
  module: Module,
) -> Result<(), CoreError> {
  tracing::debug!("Resolving tree for module {:?}", &module.location);

  for (index, dependency) in module.dependencies.iter().enumerate() {
    tracing::debug!("Processing dependency {} of {:?}", index, &module.location);

    if let Some(location) = dependency.location() {
      if cache.contains_key(&location) {
        tracing::debug!("Skipping cache key {:?}", &location);
        continue;
      }

      match dependency.asset() {
        Asset::NodePackage {
          target_file,
          package_directory,
        } => {
          let mut module = process_package_json(&resolver, package_directory)?;
          // this has two cache entries one for the dependency itself and one for the package
          tracing::debug!("Inserting: {:?} into {:?}", module.kind, &target_file);

          if target_file != package_directory {
            // This is to handle the case where we import a specific file in the node module
            tracing::debug!("Inserting: {:?} into {:?}", module.kind, &package_directory);
            cache.insert(package_directory.clone(), module.clone());
          }

          cache.insert(target_file.clone(), module.clone());

          recursively_build_dependency_tree(cache, resolver, module);
        }
        Asset::Asset(path) => {
          tracing::debug!("{:?} is an asset. No expansion required", &path)
          // stop. No further expansion here
        }
        Asset::Module(path) => {
          let next_module = process_javascript_file(&resolver, path)?;
          tracing::debug!("Inserting: {:?} into {:?}", module.kind, &path);
          cache.insert(path.clone(), next_module.clone());
          recursively_build_dependency_tree(cache, resolver, next_module);
        }
        Asset::Unresolved(path) => {
          tracing::debug!("{:?} could not be resolved", &path)
          // stop. No further expansion here
        }
      }
    } else {
      tracing::debug!("{:?} could not be resolved", &dependency);
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use std::path::{Path, PathBuf};

  use crate::{build_dependency_cache, start_resolve_project};

  #[test]
  fn the_big_one() {
    let resolver = start_resolve_project(
      Path::new("/home/kevin_velasco/data_disk/Projects/mathspace/mathspace"),
      vec![PathBuf::from("local_modules")],
    )
    .unwrap();

    let target =
            Path::new("/home/kevin_velasco/data_disk/Projects/mathspace/mathspace/local_modules/ms-pages/Teacher/Teacher.jsx");
    let mut cache = HashMap::new();
    build_dependency_cache(&resolver, &target, &mut cache);
  }
}
