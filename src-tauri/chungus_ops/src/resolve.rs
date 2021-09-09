use crate::error::CoreError;
use crate::file::process_package_json;
use crate::module::{Asset, Dependency, Location, Module, ModuleKind, RootModule};
use crate::parser::Import::NodeDependency;
use crate::parser::{Import, UnresolvedImport};
use nom::error::dbg_dmp;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{Error, Read};
use std::iter::FromIterator;
use std::ops::{Deref, DerefMut};
use std::path::{Iter, Path, PathBuf};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialOrd, PartialEq)]
enum SearchSpace {
  NodeModule(PathBuf),
  RelativePath(PathBuf),
  IncludedPath(PathBuf),
}

impl Deref for SearchSpace {
  type Target = Path;

  fn deref(&self) -> &Self::Target {
    match self {
      SearchSpace::NodeModule(p) => p,
      SearchSpace::RelativePath(p) => p,
      SearchSpace::IncludedPath(p) => p,
    }
  }
}

impl Default for Resolver {
  fn default() -> Self {
    Self {
      recursively_resolve_node_modules: false,
      resolve_root: Default::default(),
      extensions: ["jsx", "js", "ts", "tsx"]
        .iter()
        .map(|str| str.to_string())
        .collect(),
      included_directories: vec![],
    }
  }
}

pub struct Resolver {
  pub recursively_resolve_node_modules: bool,
  pub resolve_root: Location,
  pub extensions: HashSet<String>,
  pub included_directories: Vec<PathBuf>,
}

impl Resolver {
  pub fn new(resolve_root: &Location, included_directories: Vec<PathBuf>) -> Self {
    Self {
      resolve_root: resolve_root.clone(),
      included_directories,
      ..Self::default()
    }
  }

  pub fn resolve_package_json_dependencies(
    &self,
    package_json_location: &Location,
    package_json_value: &serde_json::Value,
  ) -> Vec<Dependency> {
    if self.recursively_resolve_node_modules {
      package_json_value["dependencies"]
        .as_object()
        .map(|map| {
          map
            .iter()
            .map(|(k, v)| {
              if let Some(value) = v.as_str() {
                if value.starts_with("file:") {
                  UnresolvedImport(NodeDependency(PathBuf::from(value.replace("file:", ""))))
                } else {
                  UnresolvedImport(NodeDependency(PathBuf::from(k)))
                }
              } else {
                UnresolvedImport(NodeDependency(PathBuf::from(k)))
              }
            })
            .map(|unresolved| self.resolve_asset(&package_json_location, &unresolved))
            .collect()
        })
        .unwrap_or_default()
    } else {
      vec![]
    }
  }

  pub fn resolve_normal_module(
    &self,
    location: &Location,
    dependencies: &[UnresolvedImport],
  ) -> Module {
    let deps = dependencies
      .iter()
      .map(|dependency| self.resolve_asset(&location, &dependency))
      .collect();

    Module {
      kind: ModuleKind::NormalModule,
      dependencies: deps,
      location: location.clone(),
    }
  }

  #[tracing::instrument(skip(self, location, unresolved_dependency))]
  fn resolve_asset(
    &self,
    location: &Location,
    unresolved_dependency: &UnresolvedImport,
  ) -> Dependency {
    let search_space = self.create_search_space(&location, &unresolved_dependency);
    let mut output_asset = Asset::Unresolved(unresolved_dependency.as_ref().to_path_buf());
    for path in search_space {
      // try to resolve it as a file first
      if let Some(asset) = self.resolve_file(&path).or(self.resolve_directory(&path)) {
        output_asset = asset;
        break;
      }
    }

    tracing::debug!("Resolved asset: {:?}", &output_asset);

    match unresolved_dependency.import_kind() {
      Import::Require(_) => Dependency::Require(output_asset),
      Import::AsyncImport(_) => Dependency::AsyncImport(output_asset),
      Import::ExportFrom(_) => Dependency::Import(output_asset),
      Import::Import(_) => Dependency::Import(output_asset),
      Import::NodeDependency(_) => Dependency::Import(output_asset),
    }
  }

  #[tracing::instrument(skip(self, path))]
  fn find_closest_package_json(&self, path: &Path) -> Option<Location> {
    for ancestor in path.ancestors() {
      tracing::trace!("Searching for package.json in {:?}", &ancestor);
      let package_json = ancestor.join("package.json");

      if let Ok(location) = Location::new(&package_json) {
        tracing::trace!("Found package.json in {:?}", &package_json);
        return Some(location);
      }
    }
    None
  }

  #[tracing::instrument(skip(self))]
  fn resolve_directory(&self, search_space: &SearchSpace) -> Option<Asset> {
    tracing::trace!("Resolving directory");
    if !search_space.is_dir() {
      return None;
    }

    // first we check if it's a node module or has a package json
    match &search_space {
      SearchSpace::NodeModule(path) => {
        if let Some(package_json_file) = self.find_closest_package_json(&path) {
          let location = Location::new(&**search_space).unwrap();
          let main_location = process_package_json(&self, &package_json_file)
            .map(|m| m.location)
            .ok();
          let mut file_in_directory = None;

          if Some(path.as_path()) != package_json_file.as_ref().parent() {
            // this is a case where we know for certain that we are referencing a file inside a node module;
            for extension in self.extensions.iter() {
              let file_name = Path::new("index").with_extension(extension);
              let file_path = search_space.join(file_name);
              if file_path.exists() {
                tracing::trace!("Resolved as relative module at {:?}", &file_path);
                file_in_directory = Some(Location::new(file_path).unwrap());
                break;
              }
            }
          }

          tracing::trace!("Resolved as node module at {:?}", &location);
          return Some(Asset::NodePackage {
            target_file: file_in_directory
              .or(main_location)
              .unwrap_or(location.clone()),
            package_directory: package_json_file,
          });
        }
      }
      SearchSpace::RelativePath(path) | SearchSpace::IncludedPath(path) => {
        for extension in self.extensions.iter() {
          let file_name = Path::new("index").with_extension(extension);
          let file_path = path.join(file_name);
          if file_path.exists() {
            tracing::trace!("Resolved as relative module at {:?}", &file_path);
            return Some(Asset::Module(Location::new(file_path).unwrap()));
          }
        }
      }
    }

    None
  }
  #[tracing::instrument(skip(self))]
  fn resolve_file(&self, search_space: &SearchSpace) -> Option<Asset> {
    tracing::trace!("Resolving file");
    // Files like .gif, .svg, .css etc
    if search_space.is_file()
      && !self.extensions.contains(
        &*search_space
          .extension()
          .unwrap_or_default()
          .to_string_lossy(),
      )
    {
      return Some(Asset::Asset(Location::new(&**search_space).unwrap()));
    }

    // defined extensions, check to see if any of them exist
    for extension in self.extensions.iter() {
      let file = search_space.with_extension(&extension);
      if file.exists() {
        match search_space {
          SearchSpace::NodeModule(path) => {
            if let Some(package_json) = self.find_closest_package_json(&path) {
              return Some(Asset::NodePackage {
                package_directory: Location::new(package_json).unwrap(),
                target_file: Location::new(file).unwrap(),
              });
            }
          }
          SearchSpace::RelativePath(path) | SearchSpace::IncludedPath(path) => {
            return Some(Asset::Module(Location::new(file).unwrap()))
          }
        }
      }
    }

    None
  }

  fn create_search_space<'a, 'b>(
    &'a self,
    location: &'a Location,
    target_path: &'b UnresolvedImport,
  ) -> Box<dyn Iterator<Item = SearchSpace> + 'a> {
    if target_path.as_ref().is_absolute() {
      return Box::new(std::iter::once(SearchSpace::RelativePath(
        target_path.as_ref().to_path_buf(),
      )));
    }

    let iterator = std::iter::empty();

    // the single relative path like "a/b.js"
    let iterator = if location.as_ref().is_file() {
      iterator.chain(std::iter::once(SearchSpace::RelativePath(
        location
          .as_ref()
          .parent()
          .unwrap_or(&Path::new("/"))
          .join(&target_path),
      )))
    } else {
      iterator.chain(std::iter::once(SearchSpace::RelativePath(
        location.as_ref().join(&target_path),
      )))
    };

    // all of the possible included directories
    let resolve_root = self.resolve_root.clone();
    let next_target_path = target_path.clone();

    // we should always include the resolve root
    let iterator = iterator.chain(std::iter::once(SearchSpace::RelativePath(
      resolve_root.as_ref().join(&target_path),
    )));

    let iterator = iterator.chain(self.included_directories.iter().map(move |c| {
      SearchSpace::IncludedPath(
        resolve_root
          .as_ref()
          .join(c)
          .join(next_target_path.as_ref()),
      )
    }));

    // all of the possible node modules
    let target_path = target_path.as_ref().to_path_buf();
    let iterator = iterator.chain(
      location
        .as_ref()
        .ancestors()
        .skip(1)
        .map(move |anc| SearchSpace::NodeModule(anc.join("node_modules").join(&target_path))),
    );

    Box::new(iterator)
  }
}

#[cfg(test)]
mod tests {
  use crate::module::Location;
  use crate::parser::Import::Import;
  use crate::parser::UnresolvedImport;
  use crate::resolve::Resolver;
  use std::path::{Path, PathBuf};

  #[test]
  fn it_creates_a_search_space_correctly() {
    let location = Location::new_from_path_buf(
      Path::new(env!("CARGO_MANIFEST_DIR")).join("./src/sample_javascript/module/a.js"),
    );

    let unresolved_module = UnresolvedImport::new_from_import(Import("./b".into()));

    let resolve = Resolver {
      resolve_root: unsafe { Location::new_unchcked(PathBuf::from(env!("CARGO_MANIFEST_DIR"))) },
      included_directories: vec!["local_modules".into()],
      ..Default::default()
    };

    let search_space = resolve.create_search_space(&location, &unresolved_module);

    for path in search_space {
      dbg!(path);
    }
  }

  #[test]
  fn it_can_resolve_a_bunch_of_dependencies() {
    let location = Location::new_from_path_buf(
      Path::new(env!("CARGO_MANIFEST_DIR")).join("./src/sample_javascript/module/a.js"),
    );

    let unresolved_module = [
      UnresolvedImport::new_from_import(Import("babel-polyfill".into())),
      UnresolvedImport::new_from_import(Import("babel-polyfill/lib/index".into())),
      UnresolvedImport::new_from_import(Import("../c".into())),
      UnresolvedImport::new_from_import(Import("./b".into())),
    ];

    let resolve = Resolver {
      resolve_root: unsafe {
        Location::new_unchcked(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sample_javascript"))
      },
      included_directories: vec!["ms-pages".into()],
      ..Default::default()
    };

    for module in unresolved_module {
      let resolved_dependency = resolve.resolve_asset(&location, &module);
      dbg!(resolved_dependency);
    }
  }
}
