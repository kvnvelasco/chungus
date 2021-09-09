use chungus_ops::file::FileTree;
use chungus_ops::module::Location;
use chungus_ops::resolve::Resolver;
use chungus_ops::webpack_report::WebpackReport;
use chungus_ops::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default)]
pub struct State {
  pub root_tree: Option<FileTree>,
  pub active_directory: Option<Location>,
  pub active_resolver: Resolver,
  pub active_entrypoint: Option<Location>,
  pub webpack_statistics: Option<WebpackReport>,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct SerialisableState {
  active_directory: Option<Location>,
  active_entrypoint: Option<Location>,
  extensions: Vec<String>,
  webpack_context: Option<Location>,
  root_tree: Option<FileTree>,
}

#[tauri::command(async)]
pub fn get_application_state(
  application_state: tauri::State<Arc<RwLock<State>>>,
) -> SerialisableState {
  tracing::info!("getting application state");
  let app_state = application_state.read();
  let resolver = &app_state.active_resolver;

  SerialisableState {
    active_directory: app_state.active_directory.clone(),
    active_entrypoint: app_state.active_entrypoint.clone(),
    extensions: resolver
      .extensions
      .iter()
      .map(|item| item.to_string())
      .collect(),
    webpack_context: Some(resolver.resolve_root.clone()),
    root_tree: app_state.root_tree.clone(),
  }
}
