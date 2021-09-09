use std::sync::Arc;

use chungus_ops::error::CoreError;
use chungus_ops::file::FileTree;
use chungus_ops::module::Location;
use chungus_ops::resolve::Resolver;
use chungus_ops::RwLock;

use crate::application_state::State;
use crate::loading::Loading;

#[tauri::command(async)]
#[tracing::instrument(skip(application_state, window))]
pub fn create_root_tree(
  window: tauri::Window,
  application_state: tauri::State<'_, Arc<RwLock<State>>>,
) -> Result<(), String> {
  tracing::info!("Producing file tree");
  let loading = Loading::start(&window);

  let start_path = {
    let app_state = application_state.read();
    app_state
      .active_directory
      .clone()
      .unwrap_or(Location::new(&dirs::home_dir().unwrap())?)
  };
  loading.message(&format!("Opening {:?}", &start_path));

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
