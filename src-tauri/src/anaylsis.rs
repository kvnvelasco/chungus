use std::sync::Arc;

use chungus_ops::analysis::Analysis;
use chungus_ops::module::Location;
use chungus_ops::{DependencyCache, RwLock};

use crate::application_state::State;
use crate::loading::Loading;

#[tauri::command(async)]
pub fn create_entrypoint_analysis(
  window: tauri::Window,
  application_state: tauri::State<Arc<RwLock<State>>>,
  dependency_analysis: tauri::State<Arc<RwLock<DependencyAnalysis>>>,
  entrypoint: String,
) -> Result<(), String> {
  tracing::info!("Creating entrypoint analysis");
  let loading = Loading::start(&window);

  dependency_analysis.write().analysis = None;
  window.emit("entrypoint_analysis::sync", "");

  let location = Location::new(&entrypoint)?;
  let mut app_state = application_state.read();
  let resolver = &app_state.active_resolver;

  let mut dependency_cache = dependency_analysis.write();
  let cache = &mut dependency_cache.cache;

  loading.message("Building dependency cache");

  chungus_ops::build_dependency_cache(resolver, &location, cache, &loading)?;

  loading.message("Starting Analysis");
  let mut analysis = Analysis::create_from_cache(&resolver, &cache, &location, &loading)?;
  loading.message("Created Analysis");
  tracing::info!("Created entrypoint analysis");

  if let Some(ref webpack) = app_state.webpack_statistics {
    loading.message("Augmenting with webpack");
    analysis.augment_with_webpack_report(&webpack, 0);
  }

  dependency_cache.analysis = Some(analysis);

  loading.message("Synchronizing");
  loading.close();

  window.emit("entrypoint_analysis::sync", "");

  Ok(())
}

#[tauri::command(async)]
pub fn get_entrypoint_analysis(
  window: tauri::Window,
  dependency_analysis: tauri::State<Arc<RwLock<DependencyAnalysis>>>,
) -> Option<serde_json::Value> {
  let logging = Loading::start(&window);
  let read_guard = dependency_analysis.read();
  logging.message("Serializing state");
  let out = read_guard
    .analysis
    .as_ref()
    .map(|analysis| serde_json::to_value(analysis.clone()).unwrap());
  logging.message("Sending");

  out
}

#[derive(Default)]
pub struct DependencyAnalysis {
  pub cache: DependencyCache,
  pub analysis: Option<Analysis>,
}
