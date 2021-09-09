use crate::error::CoreError;
use crate::module::{Location, Module, ModuleKind, RelativePath};
use crate::parser::Import::NodeDependency;
use crate::parser::UnresolvedImport;
use crate::resolve::Resolver;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileTree {
  root_path: Location,
  file_node: Arc<FileNode>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FileNode {
  is_folder: bool,
  path: PathBuf,
  valid_entrypoint: bool,
  full_path: PathBuf,
  children: Vec<FileNode>,
}

pub fn find_highest_path(mut paths: impl Iterator<Item = Location>) -> Option<Location> {
  let mut highest = paths.next()?;
  for path in paths {
    if highest.as_ref().starts_with(&path) {
      highest = path
    }
  }
  Some(highest)
}

impl FileNode {
  fn traverse_self(
    resolve: &Resolver,
    root_path: &Location,
    path: &Location,
    // only open the following files
    filter: &Option<HashSet<Location>>,
  ) -> Result<Self, CoreError> {
    if path.as_ref().is_dir() {
      let dir_entries = std::fs::read_dir(path.as_ref())?;
      let mut output = vec![];
      for dir in dir_entries {
        let entry = dir?;
        let entry_path = entry.path();
        if let Some(filters) = filter {
          let mut contained = false;
          for filter in filters.iter() {
            if filter.as_ref().starts_with(&Location::new(&entry_path)?) {
              contained = true;
              break;
            }
          }

          if !contained {
            continue;
          }
        }
        if entry_path.is_dir() {
          output.push(Self::traverse_self(
            &resolve,
            &path,
            &Location::new(entry_path).unwrap(),
            &filter,
          )?)
        } else {
          output.push(Self {
            valid_entrypoint: entry_path
              .extension()
              .map(|ext| resolve.extensions.contains(&*ext.to_string_lossy()))
              .unwrap_or(false),
            path: entry_path.strip_prefix(&path).unwrap().to_path_buf(),
            full_path: entry_path,
            children: vec![],
            is_folder: false,
          })
        }
      }
      Ok(Self {
        children: output,
        full_path: path.as_ref().to_path_buf(),
        valid_entrypoint: false,
        path: path
          .as_ref()
          .strip_prefix(&root_path)
          .unwrap()
          .to_path_buf(),
        is_folder: true,
      })
    } else {
      Ok(Self {
        children: vec![],
        full_path: path.as_ref().to_path_buf(),
        valid_entrypoint: path
          .as_ref()
          .extension()
          .map(|ext| resolve.extensions.contains(&*ext.to_string_lossy()))
          .unwrap_or(false),
        path: path
          .as_ref()
          .strip_prefix(&root_path)
          .unwrap()
          .to_path_buf(),
        is_folder: false,
      })
    }
  }
}

impl FileTree {
  pub fn open_from_root_path(
    resolve: &Resolver,
    path: &Path,
    filterset: &Option<HashSet<Location>>,
  ) -> Result<Self, CoreError> {
    let location = Location::new(path)?;

    Ok(Self {
      file_node: Arc::new(FileNode::traverse_self(
        &resolve, &location, &location, filterset,
      )?),
      root_path: location,
    })
  }
}

#[tracing::instrument(skip(resolve))]
pub fn process_javascript_file(
  resolve: &Resolver,
  file_location: &Location,
) -> Result<Module, CoreError> {
  let location = file_location;
  let file_contents = {
    let mut file_handle = OpenOptions::new().read(true).open(location.as_ref())?;
    let mut contents = String::new();
    file_handle.read_to_string(&mut contents)?;
    contents
  };

  let unresolved_dependencies = UnresolvedImport::parse_many(&file_contents)?;
  let module = resolve.resolve_normal_module(&location, &unresolved_dependencies);

  tracing::trace!(
    "Created javascript normal module with {} dependencies",
    module.dependencies.len()
  );
  Ok(module)
}

#[tracing::instrument(skip(resolve))]
pub fn process_package_json(
  resolve: &Resolver,
  package_json_location: &Location,
) -> Result<Module, CoreError> {
  let file = OpenOptions::new()
    .read(true)
    .open(package_json_location.as_ref())?;
  let reader = BufReader::new(file);

  let value: serde_json::Value = serde_json::from_reader(reader).unwrap();
  let main_file = value["module"]
    .as_str()
    .or(value["main"].as_str())
    .unwrap_or("index.js");
  let main_file_path = Location::new(
    package_json_location
      .as_ref()
      .parent()
      .unwrap()
      .join(&main_file),
  )?;

  let dependencies = resolve.resolve_package_json_dependencies(&package_json_location, &value);
  tracing::trace!(
    "Created javascript node module with {} dependencies",
    dependencies.len()
  );
  Ok(Module {
    kind: ModuleKind::NodeModule,
    dependencies,
    location: main_file_path,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::module::Location;
  use crate::resolve::Resolver;
  use std::path::Path;
  #[test]
  fn it_can_read_a_package_json_file() {
    let location = Location::new(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let resolve = Resolver::new(&location, vec![]);
    let module = process_package_json(
      &resolve,
      &Location::new(
        &location
          .as_ref()
          .join("./src/sample_javascript/node_modules/babel-polyfill/package.json"),
      )
      .unwrap(),
    );

    dbg!(module);
  }

  #[test]
  fn it_can_resolve_a_javascript_file() {
    let location = Location::new(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let resolve = Resolver::new(&location, vec![]);

    let module = process_javascript_file(
      &resolve,
      &Location::new(
        &location
          .as_ref()
          .join("./src/sample_javascript/module/a.js"),
      )
      .unwrap(),
    );

    dbg!(module);
  }
}
