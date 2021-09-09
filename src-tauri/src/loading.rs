use chungus_ops::logging::ClientSideLogger;

pub struct Loading<'a, T: tauri::Runtime> {
  window: &'a tauri::Window<T>,
}

impl<'a, T: tauri::Runtime> Loading<'a, T> {
  pub fn start(window: &'a tauri::Window<T>) -> Self {
    window.emit("loading::true", "");
    Self { window }
  }
  pub fn message(&self, message: &str) {
    self.window.emit("loading::message", message);
  }
  pub fn close(self) {
    self.window.emit("loading::false", "");
  }
}

impl<'a, T: tauri::Runtime> Drop for Loading<'a, T> {
  fn drop(&mut self) {
    self.window.emit("loading::false", "");
  }
}

impl<'a, T: tauri::Runtime> ClientSideLogger for Loading<'a, T> {
  fn message(&self, message: impl AsRef<str>) {
    self.message(message.as_ref())
  }
}
