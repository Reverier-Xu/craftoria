use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ShellError {
  #[error("failed to parse the log filter from the environment: {0}")]
  LogFilter(#[from] tracing_subscriber::filter::FromEnvError),
  #[error("failed to install the global tracing subscriber: {0}")]
  SubscriberInit(#[from] tracing::subscriber::SetGlobalDefaultError),
}
