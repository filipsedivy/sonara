//! The `rmcp` server handler and its stdio runtime.
//!
//! The handler answers `initialize`, `tools/list` and `tools/call` (and the SDK answers `ping`).
//! Its replies are built from the JSON of the parent module: `rmcp`'s model types are
//! non-exhaustive, so they are read from that JSON instead of being assembled field by field,
//! which keeps the golden-checked JSON the one source of truth.

use std::io;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ErrorCode, Implementation,
    InitializeResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::{MaybeSendFuture, RequestContext, ServerInitializeError};
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::Value;
use sonara_core::error::DomainError;

use super::{INSTRUCTIONS, ToolBackend, call_result, capabilities, listed_tool, server_info};

/// The MCP server could not start or stopped abnormally.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// The `tokio` runtime could not be started.
    #[error("cannot start the MCP runtime: {0}")]
    Runtime(#[source] io::Error),
    /// The server's JSON does not fit the SDK's model (a defect, not an operational error).
    #[error("the MCP reply does not fit the SDK model: {0}")]
    Model(#[from] serde_json::Error),
    /// The `initialize` handshake failed.
    #[error("MCP initialization failed: {0}")]
    Initialize(#[source] Box<ServerInitializeError>),
    /// The server task ended abnormally.
    #[error("the MCP server task failed: {0}")]
    Task(#[source] tokio::task::JoinError),
    /// The backend could not list its tools.
    #[error("cannot list the tools: {0}")]
    Backend(#[source] DomainError),
}

/// The JSON-RPC error code of a tool call that raised: the prototype's SDK answers an exception
/// in a handler with code 0 and the exception's text.
const HANDLER_ERROR: ErrorCode = ErrorCode(0);

/// Serves the tools of `backend` over MCP on stdin and stdout until the client closes stdin.
/// Nothing else may write to stdout meanwhile.
///
/// A client that closes stdin before the `initialize` handshake ends the server normally.
///
/// # Errors
///
/// [`McpError`] when the backend cannot list its tools, the runtime cannot start, the handshake
/// fails, the server task fails, or (a defect) the server's JSON does not fit the SDK's model.
pub fn run_stdio(backend: Arc<dyn ToolBackend>) -> Result<(), McpError> {
    let server = SonaraServer::new(backend)?;
    // One thread for the protocol; tool calls run on the runtime's blocking threads. `rmcp`
    // uses timers, so every driver is enabled.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(McpError::Runtime)?;
    runtime.block_on(async move {
        let running = match server.serve(rmcp::transport::stdio()).await {
            Ok(running) => running,
            Err(ServerInitializeError::ConnectionClosed(_)) => return Ok(()),
            Err(error) => return Err(McpError::Initialize(Box::new(error))),
        };
        running.waiting().await.map_err(McpError::Task)?;
        Ok(())
    })
}

/// The handler: the backend, its tool listing and the `initialize` reply.
struct SonaraServer {
    backend: Arc<dyn ToolBackend>,
    tools: Vec<Tool>,
    config: ServerConfig,
}

impl SonaraServer {
    /// The handler for `backend`, listing the tools the backend reports now (the prototype
    /// reads them once, when it builds the server).
    fn new(backend: Arc<dyn ToolBackend>) -> Result<Self, McpError> {
        let tools = backend
            .tools()
            .map_err(McpError::Backend)?
            .iter()
            .map(|spec| serde_json::from_value::<Tool>(listed_tool(spec)))
            .collect::<Result<Vec<_>, _>>()?;
        let capabilities: ServerCapabilities = serde_json::from_value(capabilities())?;
        let info: Implementation = serde_json::from_value(server_info())?;
        // The handshake answers the client's version when the SDK knows it, else this one: the
        // newest version with an `initialize` handshake, as the prototype's SDK does.
        let config = InitializeResult::new(capabilities)
            .with_server_info(info)
            .with_instructions(INSTRUCTIONS)
            .with_protocol_version(ProtocolVersion::LATEST_WITH_INITIALIZE);
        Ok(Self {
            backend,
            tools,
            config,
        })
    }
}

impl ServerHandler for SonaraServer {
    fn get_info(&self) -> ServerConfig {
        self.config.clone()
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(self.tools.clone())))
    }

    /// Every call reaches the backend, an unknown tool included: its error is an envelope with
    /// `isError`, as in the prototype, not a protocol error. Only a call that cannot reach the
    /// tools (a running session that does not answer) is a protocol error, with the code and
    /// text the prototype's SDK gives a raising handler.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let backend = Arc::clone(&self.backend);
        let name = request.name.into_owned();
        // `dict(params.arguments or {})`.
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let envelope = tokio::task::spawn_blocking(move || backend.call(&name, Some(&arguments)))
            .await
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?
            .map_err(|error| ErrorData::new(HANDLER_ERROR, error.to_string(), None))?;
        let result: CallToolResult = serde_json::from_value(call_result(&envelope))
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        Ok(result.into())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::ServerHandler;
    use serde_json::{Value, json};
    use sonara_core::error::DomainError;

    use super::SonaraServer;
    use crate::contract::{TOOLS, ToolSpec};
    use crate::mcp::{
        INSTRUCTIONS, ToolBackend, call_result, capabilities, server_info, tool, tools,
    };

    /// Lists the whole contract and answers every call with an echo.
    struct Contract;

    impl ToolBackend for Contract {
        fn call(&self, name: &str, arguments: Option<&Value>) -> Result<Value, DomainError> {
            Ok(json!({"ok": name == "undo", "tool": name, "arguments": arguments}))
        }

        fn tools(&self) -> Result<Vec<Value>, DomainError> {
            Ok(TOOLS.iter().map(ToolSpec::to_json).collect())
        }
    }

    #[test]
    fn the_sdk_model_keeps_the_listing_and_the_handshake() {
        let server = SonaraServer::new(Arc::new(Contract)).unwrap();
        let listed: Vec<Value> = server
            .tools
            .iter()
            .map(|t| serde_json::to_value(t).unwrap())
            .collect();
        assert_eq!(listed, tools());
        let info = serde_json::to_value(server.get_info()).unwrap();
        assert_eq!(
            info,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": capabilities(),
                "serverInfo": server_info(),
                "instructions": INSTRUCTIONS,
            })
        );
        assert_eq!(listed.first(), TOOLS.first().map(tool).as_ref());
    }

    #[test]
    fn the_sdk_model_keeps_call_results() {
        for envelope in [
            json!({"ok": true, "tool": "undo", "x": 1.5}),
            json!({"ok": false, "tool": "x", "error": {"code": "NOT_FOUND", "message": "m"}}),
        ] {
            let result: rmcp::model::CallToolResult =
                serde_json::from_value(call_result(&envelope)).unwrap();
            assert_eq!(
                serde_json::to_value(&result).unwrap(),
                call_result(&envelope)
            );
        }
    }
}
