#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

use tauri::{CustomMenuItem, Manager, Menu, Submenu};
use tracing::subscriber::set_global_default;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, FmtSubscriber, Registry};

use chungus_ops::RwLock;

use crate::anaylsis::DependencyAnalysis;
use crate::application_state::State;
use std::sync::Arc;

mod anaylsis;
mod application_state;
mod file_tree;
mod loading;
mod webpack;

use anaylsis::*;
use application_state::*;
use file_tree::*;
use webpack::*;

fn main() {
  let registry = Registry::default()
    .with(EnvFilter::from_default_env())
    .with(tracing_subscriber::fmt::layer());
  set_global_default(registry);

  tauri::Builder::default()
    .manage(Arc::new(RwLock::new(State::default())))
    .manage(Arc::new(RwLock::new(DependencyAnalysis::default())))
    .invoke_handler(tauri::generate_handler![
      create_root_tree,
      get_application_state,
      create_entrypoint_analysis,
      get_entrypoint_analysis,
      create_webpack_statstics,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
