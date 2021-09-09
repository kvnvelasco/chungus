#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{CustomMenuItem, Manager, Menu, Submenu};

use chungus_ops::analysis::Analysis;
use chungus_ops::error::CoreError;
use chungus_ops::file::FileTree;
use chungus_ops::module::Location;
use chungus_ops::resolve::Resolver;
use chungus_ops::{DependencyCache, RwLock};

use chungus_ops::webpack_report::WebpackReport;
use std::fs::OpenOptions;
use std::io::{BufReader, Read};
use tracing::subscriber::set_global_default;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, FmtSubscriber, Registry};

#[derive(Default)]
struct DependencyAnalysis {
  cache: DependencyCache,
  analysis: Option<Analysis>,
}

#[derive(Default)]
struct State {
  root_tree: Option<FileTree>,
  active_directory: Option<Location>,
  active_resolver: Resolver,
  active_entrypoint: Option<Location>,
  webpack_statistics: Option<WebpackReport>,
}

#[derive(Default, Serialize, Deserialize, Clone)]
struct SerialisableState {
  active_directory: Option<Location>,
  active_entrypoint: Option<Location>,
  extensions: Vec<String>,
  webpack_context: Option<Location>,
  root_tree: Option<FileTree>,
}

#[tauri::command(async)]
fn create_webpack_statstics(
  window: tauri::Window,
  application_state: tauri::State<Arc<RwLock<State>>>,
  dependency_analysis: tauri::State<Arc<RwLock<DependencyAnalysis>>>,
) -> Result<(), String> {
  let start_path = {
    let app_state = application_state.read();
    app_state
      .active_directory
      .clone()
      .unwrap_or(Location::new(&dirs::home_dir().unwrap())?)
  };

  let path = native_dialog::FileDialog::new()
    .add_filter("Json files", &["json"])
    .set_location(&start_path);

  let path = path
    .show_open_single_file()
    .map_err(|e| CoreError::custom(&e.to_string()))?;

  if let Some(path) = path {
    let app_state = application_state.read();

    let resolver = &app_state.active_resolver;
    let webpack_file_location = Location::new(&path)?;
    let file = OpenOptions::new()
      .read(true)
      .open(&webpack_file_location)
      .map_err(Into::<CoreError>::into)?;

    let reader = BufReader::new(file);
    let report = chungus_ops::webpack_report::v4::create_report_from_reader(resolver, reader)?;
    drop(app_state);
    {
      let mut dependency_cache = dependency_analysis.write();
      if let Some(analysis) = &mut dependency_cache.analysis {
        analysis.augment_with_webpack_report(&report, 0);
        window.emit("entrypoint_analysis::sync", "");
      }
    }

    {
      let mut guard = application_state.write();
      guard.webpack_statistics = Some(report);
    }

    Ok(())
  } else {
    Err(CoreError::custom("Could not build webpack statistics").to_string())
  }
}

#[tauri::command(async)]
fn create_entrypoint_analysis(
  window: tauri::Window,
  application_state: tauri::State<Arc<RwLock<State>>>,
  dependency_analysis: tauri::State<Arc<RwLock<DependencyAnalysis>>>,
  entrypoint: String,
) -> Result<(), String> {
  tracing::info!("Creating entrypoint analysis");

  let location = Location::new(&entrypoint)?;
  let mut app_state = application_state.read();
  let resolver = &app_state.active_resolver;

  let mut dependency_cache = dependency_analysis.write();

  let cache = &mut dependency_cache.cache;
  chungus_ops::build_dependency_cache(resolver, &location, cache)?;

  let mut analysis = Analysis::create_from_cache(&resolver, &cache, &location)?;
  tracing::info!("Created entrypoint analysis");

  if let Some(ref webpack) = app_state.webpack_statistics {
    analysis.augment_with_webpack_report(&webpack, 0);
  }
  dependency_cache.analysis = Some(analysis);

  window.emit("entrypoint_analysis::sync", "");

  Ok(())
}

#[tauri::command(async)]
fn get_entrypoint_analysis(
  dependency_analysis: tauri::State<Arc<RwLock<DependencyAnalysis>>>,
) -> Option<Analysis> {
  let read_guard = dependency_analysis.read();
  read_guard
    .analysis
    .as_ref()
    .map(|analysis| analysis.clone())
}

#[tauri::command(async)]
fn get_application_state(application_state: tauri::State<Arc<RwLock<State>>>) -> SerialisableState {
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

#[tauri::command(async)]
#[tracing::instrument(skip(application_state, window))]
fn create_root_tree(
  window: tauri::Window,
  application_state: tauri::State<'_, Arc<RwLock<State>>>,
) -> Result<(), String> {
  tracing::info!("Producing file tree");

  let start_path = {
    let app_state = application_state.read();
    app_state
      .active_directory
      .clone()
      .unwrap_or(Location::new(&dirs::home_dir().unwrap())?)
  };

  let handle = {
    let path = native_dialog::FileDialog::new().set_location(&start_path);

    let path = path
      .show_open_single_dir()
      .map_err(|e| CoreError::custom(&e.to_string()))?;

    if let Some(path) = path {
      let app_state = application_state.read();

      let resolver = &app_state.active_resolver;

      Ok((
        FileTree::open_from_root_path(&resolver, &path, &None).map_err(|e| e.to_string()),
        path,
      ))
    } else {
      Err(CoreError::custom("Did not open a directory").to_string())
    }
  };

  let (result, path) = handle?;
  let location = Location::new(&path)?;
  let resolver = Resolver::new(&location, vec![]);

  {
    let mut lock = application_state.write();
    let file_tree = result?;
    lock.root_tree = Some(file_tree);
    lock.active_directory = Some(Location::new(&path)?);
    lock.active_resolver = resolver;
  }

  tracing::info!("Propagating updated state");
  window.emit("application_state::sync", "");

  Ok(())
}

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
