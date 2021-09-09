pub trait ClientSideLogger {
  fn message(&self, message: impl AsRef<str>);
}
