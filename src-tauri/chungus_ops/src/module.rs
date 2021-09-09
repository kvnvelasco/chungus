use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::path::{Path, PathBuf};

#[derive(Clone, PartialOrd, PartialEq, Eq, Serialize, Deserialize, Default, Ord)]
pub struct Location(PathBuf);

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Serialize, Deserialize, Hash, Ord)]
pub struct RelativePath(PathBuf);

impl Deref for RelativePath {
  type Target = Path;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl RelativePath {
  pub fn make_from_path(path: impl AsRef<Path>, root: &Location) -> Result<Self, CoreError> {
    if !root.as_ref().join(&path).exists() {
      Err(CoreError::custom(&format!(
        "Cannot make relative path for {:?}",
        path.as_ref()
      )))
    } else {
      Ok(Self(path.as_ref().to_path_buf()))
    }
  }
}

impl Debug for Location {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}

#[cfg(test)]
impl Location {
  pub fn new_from_path_buf(path: PathBuf) -> Self {
    Self(path)
  }
}

impl Hash for Location {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.hash(state)
  }
}

impl Location {
  pub fn new(path: impl AsRef<Path>) -> Result<Self, CoreError> {
    let path = path.as_ref();

    Ok(Self(path.canonicalize().map_err(|_e| {
      CoreError::custom(&format!("Could not resolve path {:?}", &path))
    })?))
  }

  pub fn make_relative_to(&self, location: &Location) -> Result<RelativePath, CoreError> {
    let next = pathdiff::diff_paths(&self.0, &location);
    match next {
      None => Err(CoreError::custom(&format!(
        "relative path {} cannot be made relative to {}",
        self.0.to_string_lossy(),
        location.as_ref().to_string_lossy()
      ))),
      Some(next) => Ok(RelativePath(next)),
    }
  }

  pub unsafe fn new_unchcked(path: PathBuf) -> Self {
    Self(path)
  }
}

impl AsRef<Path> for Location {
  fn as_ref(&self) -> &Path {
    &self.0
  }
}

pub struct RootModule {
  pub package_json_location: Location,
  pub target_file: Location,
  pub dependencies: Vec<Dependency>,
}

#[derive(Copy, Clone, PartialOrd, PartialEq, Debug, Eq)]
pub enum ModuleKind {
  NodeModule,
  NormalModule,
}

#[derive(Clone, Debug, PartialOrd, Eq)]
pub struct Module {
  pub location: Location,
  pub kind: ModuleKind,
  pub dependencies: Vec<Dependency>,
}

impl Hash for Module {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.location.hash(state)
  }
}

impl PartialEq for Module {
  fn eq(&self, other: &Self) -> bool {
    self.location.eq(&other.location)
  }
}

#[derive(Clone, PartialOrd, PartialEq, Hash, Eq)]
pub enum Dependency {
  Require(Asset),
  Import(Asset),
  AsyncImport(Asset),
}

impl Debug for Dependency {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.asset().fmt(f)
  }
}

impl Dependency {
  pub fn asset(&self) -> &Asset {
    match &self {
      Dependency::Require(a) => a,
      Dependency::Import(a) => a,
      Dependency::AsyncImport(a) => a,
    }
  }
  pub fn location(&self) -> Option<Location> {
    match &self {
      Dependency::Require(loc) => loc.location(),
      Dependency::Import(loc) => loc.location(),
      Dependency::AsyncImport(loc) => loc.location(),
    }
  }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Hash, Eq)]
pub enum Asset {
  NodePackage {
    package_directory: Location,
    target_file: Location,
  },
  Asset(Location),
  Module(Location),
  Unresolved(PathBuf),
}

impl Asset {
  pub fn location(&self) -> Option<Location> {
    match self {
      Asset::NodePackage { target_file, .. } => Some(target_file.clone()),
      Asset::Asset(path) => Some(path.clone()),
      Asset::Module(path) => Some(path.clone()),
      Asset::Unresolved(_) => None,
    }
  }
}
