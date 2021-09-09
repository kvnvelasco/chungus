use std::fs::OpenOptions;
use std::io::BufReader;
use std::sync::Arc;

use chungus_ops::error::CoreError;
use chungus_ops::module::Location;
use chungus_ops::RwLock;

use crate::anaylsis::DependencyAnalysis;
use crate::application_state::State;
use crate::loading::Loading;
use chungus_ops::logging::ClientSideLogger;

#[tauri::command(async)]
pub fn create_webpack_statstics(
  window: tauri::Window,
  application_state: tauri::State<Arc<RwLock<State>>>,
  dependency_analysis: tauri::State<Arc<RwLock<DependencyAnalysis>>>,
) -> Result<(), String> {
  let loading = Loading::start(&window);
  let start_path = {
    let app_state = application_state.read();
    app_state
      .active_directory
      .clone()
      .unwrap_or(Location::new(&dirs::home_dir().unwrap())?)
  };

  ClientSideLogger::message(
    &loading,
    format!("Loading webpack stats at {:?}", &start_path),
  );

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

    loading.message(&format!(
      "Parsing webpack stats file at {:?}",
      &webpack_file_location
    ));

    let file = OpenOptions::new()
      .read(true)
      .open(&webpack_file_location)
      .map_err(Into::<CoreError>::into)?;

    let reader = BufReader::new(file);
    loading.message("Creating report");
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
