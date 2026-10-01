use super::{start_mcp_http_endpoint, McpExecutionContext, McpHttpError};
use std::fmt;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use tokio::sync::oneshot;

/// Synchronous owner for the Runtime's dedicated MCP HTTP service thread.
///
/// The control server owns this alongside RuntimeCore; no RuntimeCore lock is
/// shared with MCP requests. The only shared lifecycle projection is carried
/// by McpExecutionContext's readiness gate.
pub struct McpRuntimeService {
    endpoint_url: Option<String>,
    shutdown_sender: Option<oneshot::Sender<()>>,
    completion_receiver: Option<Receiver<Result<(), McpRuntimeServiceError>>>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Debug)]
pub enum McpRuntimeServiceError {
    AlreadyRunning,
    ThreadStart { source: std::io::Error },
    RuntimeBuild { source: std::io::Error },
    Endpoint { source: McpHttpError },
    StartupChannelClosed,
    CompletionChannelClosed,
    ThreadPanicked,
}

impl fmt::Display for McpRuntimeServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => {
                formatter.write_str("the Runtime MCP service is already running")
            }
            Self::ThreadStart { .. } => {
                formatter.write_str("could not start the Runtime MCP service thread")
            }
            Self::RuntimeBuild { .. } => {
                formatter.write_str("could not create the Runtime MCP service runtime")
            }
            Self::Endpoint { .. } => {
                formatter.write_str("could not start the Runtime MCP endpoint")
            }
            Self::StartupChannelClosed => {
                formatter.write_str("the Runtime MCP service stopped during startup")
            }
            Self::CompletionChannelClosed => {
                formatter.write_str("the Runtime MCP service stopped without a completion result")
            }
            Self::ThreadPanicked => formatter.write_str("the Runtime MCP service thread panicked"),
        }
    }
}

impl std::error::Error for McpRuntimeServiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ThreadStart { source } | Self::RuntimeBuild { source } => Some(source),
            Self::Endpoint { source } => Some(source),
            Self::AlreadyRunning
            | Self::StartupChannelClosed
            | Self::CompletionChannelClosed
            | Self::ThreadPanicked => None,
        }
    }
}

impl Default for McpRuntimeService {
    fn default() -> Self {
        Self::new()
    }
}

impl McpRuntimeService {
    pub fn new() -> Self {
        Self {
            endpoint_url: None,
            shutdown_sender: None,
            completion_receiver: None,
            thread: None,
        }
    }

    /// Starts the listener and waits for the background thread's bind/startup
    /// handshake. An Ok result guarantees a bound `/mcp` endpoint and URL.
    pub fn start(&mut self, context: McpExecutionContext) -> Result<(), McpRuntimeServiceError> {
        if self.thread.is_some() {
            return Err(McpRuntimeServiceError::AlreadyRunning);
        }

        let (startup_sender, startup_receiver) = mpsc::channel();
        let (completion_sender, completion_receiver) = mpsc::channel();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let thread = thread::Builder::new()
            .name("harness-mcp-http".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()
                    .map_err(|source| McpRuntimeServiceError::RuntimeBuild { source });
                let result = match runtime {
                    Ok(runtime) => runtime.block_on(async move {
                        let mut endpoint = match start_mcp_http_endpoint(context).await {
                            Ok(endpoint) => endpoint,
                            Err(source) => {
                                let error = McpRuntimeServiceError::Endpoint { source };
                                let _ = startup_sender.send(Err(error));
                                return Ok(());
                            }
                        };
                        let url = endpoint.url();
                        if startup_sender.send(Ok(url)).is_err() {
                            return endpoint
                                .shutdown()
                                .await
                                .map_err(|source| McpRuntimeServiceError::Endpoint { source });
                        }
                        let _ = shutdown_receiver.await;
                        endpoint
                            .shutdown()
                            .await
                            .map_err(|source| McpRuntimeServiceError::Endpoint { source })
                    }),
                    Err(error) => {
                        let _ = startup_sender.send(Err(error));
                        Ok(())
                    }
                };
                let _ = completion_sender.send(result);
            })
            .map_err(|source| McpRuntimeServiceError::ThreadStart { source })?;

        self.thread = Some(thread);
        self.shutdown_sender = Some(shutdown_sender);
        self.completion_receiver = Some(completion_receiver);

        match startup_receiver.recv() {
            Ok(Ok(url)) => {
                self.endpoint_url = Some(url);
                Ok(())
            }
            Ok(Err(error)) => {
                let _ = self.join_finished_thread();
                Err(error)
            }
            Err(_) => {
                let _ = self.join_finished_thread();
                Err(McpRuntimeServiceError::StartupChannelClosed)
            }
        }
    }

    pub(crate) fn endpoint_url(&self) -> Option<&str> {
        self.endpoint_url.as_deref()
    }

    /// Stops the endpoint gracefully and joins its dedicated OS thread. It is
    /// idempotent once the service has stopped.
    pub fn shutdown(&mut self) -> Result<(), McpRuntimeServiceError> {
        let Some(shutdown_sender) = self.shutdown_sender.take() else {
            return Ok(());
        };
        let _ = shutdown_sender.send(());
        self.join_finished_thread()
    }

    fn join_finished_thread(&mut self) -> Result<(), McpRuntimeServiceError> {
        self.endpoint_url = None;
        let join_result = self.thread.take().map(|thread| {
            thread
                .join()
                .map_err(|_| McpRuntimeServiceError::ThreadPanicked)
        });
        let completion_result = self.completion_receiver.take().map(|receiver| {
            receiver
                .recv()
                .map_err(|_| McpRuntimeServiceError::CompletionChannelClosed)
        });
        self.shutdown_sender = None;

        if let Some(result) = join_result {
            result?;
        }
        match completion_result {
            Some(Ok(result)) => result,
            Some(Err(error)) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for McpRuntimeService {
    fn drop(&mut self) {
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            let _ = shutdown_sender.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::RuntimeCore;
    use harness_control_protocol::{
        DecimalU64, RuntimeConfigSnapshot, RuntimeLimits, SecurityConfig, SecurityMode, ToolConfig,
        WorkspaceConfig,
    };
    use std::fs;
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::path::PathBuf;

    #[test]
    fn starts_with_a_synchronous_handshake_and_releases_its_port() {
        let base =
            std::env::temp_dir().join(format!("harness-mcp-runtime-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&base).expect("create workspace");
        let mut core = RuntimeCore::new();
        core.initialize(config_for(&base)).expect("initialize core");
        let mut service = McpRuntimeService::new();

        service
            .start(core.mcp_context().expect("context"))
            .expect("start service");
        let url = service.endpoint_url().expect("endpoint URL").to_owned();
        let address: SocketAddr = url
            .strip_prefix("http://")
            .and_then(|value| value.strip_suffix("/mcp"))
            .expect("endpoint authority")
            .parse()
            .expect("socket address");
        assert!(TcpStream::connect(address).is_ok());
        assert!(matches!(
            service.start(core.mcp_context().unwrap()),
            Err(McpRuntimeServiceError::AlreadyRunning)
        ));

        service.shutdown().expect("shutdown service");
        assert!(service.endpoint_url().is_none());
        TcpListener::bind(address).expect("released endpoint port");
        service.shutdown().expect("idempotent shutdown");
        let _ = fs::remove_dir_all(base);
    }

    fn config_for(root: &PathBuf) -> RuntimeConfigSnapshot {
        RuntimeConfigSnapshot {
            config_revision: DecimalU64::new(1),
            workspace: WorkspaceConfig {
                root_path: root.to_string_lossy().into_owned(),
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: 1,
                max_activity_events: 10,
                max_read_bytes: 64,
            },
            tools: ToolConfig::default(),
        }
    }
}
