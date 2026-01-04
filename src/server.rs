//! MCP server implementation for agent orchestration.

use std::io::{BufRead, BufReader, Write};

use serde_json::json;
use tracing::{debug, error, info};

use crate::error::{Error, Result};
use crate::orchestrator::AgentOrchestrator;
use crate::protocol::{
    error_codes, McpRequest, McpResponse, ServerCapabilities, ServerInfo, ToolCapabilities,
};
use crate::tools::ToolRegistry;

/// Agent MCP Server.
pub struct AgentMcpServer {
    /// Tool registry.
    registry: ToolRegistry,
    /// Server info.
    server_info: ServerInfo,
    /// Whether the server is initialized.
    initialized: bool,
}

impl AgentMcpServer {
    /// Create a new MCP server.
    pub fn new(orchestrator: AgentOrchestrator) -> Self {
        Self {
            registry: ToolRegistry::new(orchestrator),
            server_info: ServerInfo::default(),
            initialized: false,
        }
    }

    /// Run the server on stdio.
    pub async fn run_stdio(&mut self) -> Result<()> {
        info!("Starting Agent MCP Server on stdio");

        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = BufReader::new(stdin.lock());

        for line in reader.lines() {
            let line = line.map_err(|e| Error::Io(e))?;
            if line.is_empty() {
                continue;
            }

            debug!("Received: {}", line);

            let response = self.handle_message(&line).await;
            let response_json = serde_json::to_string(&response)?;

            debug!("Sending: {}", response_json);

            writeln!(stdout, "{}", response_json).map_err(|e| Error::Io(e))?;
            stdout.flush().map_err(|e| Error::Io(e))?;
        }

        Ok(())
    }

    /// Handle a single message.
    async fn handle_message(&mut self, message: &str) -> McpResponse {
        // Parse request
        let request: McpRequest = match serde_json::from_str(message) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse request: {}", e);
                return McpResponse::error(None, error_codes::PARSE_ERROR, e.to_string());
            }
        };

        // Handle method
        match request.method.as_str() {
            "initialize" => self.handle_initialize(&request),
            "initialized" => self.handle_initialized(&request),
            "tools/list" => self.handle_tools_list(&request),
            "tools/call" => self.handle_tools_call(&request).await,
            "ping" => self.handle_ping(&request),
            _ => {
                McpResponse::error(
                    request.id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("unknown method: {}", request.method),
                )
            }
        }
    }

    /// Handle initialize request.
    fn handle_initialize(&mut self, request: &McpRequest) -> McpResponse {
        info!("Initializing MCP server");

        let capabilities = ServerCapabilities {
            tools: Some(ToolCapabilities { list_changed: false }),
            resources: None,
            prompts: None,
        };

        McpResponse::success(
            request.id.clone(),
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": capabilities,
                "serverInfo": self.server_info
            }),
        )
    }

    /// Handle initialized notification.
    fn handle_initialized(&mut self, request: &McpRequest) -> McpResponse {
        self.initialized = true;
        info!("MCP server initialized");

        // This is a notification, no response needed
        McpResponse::success(request.id.clone(), json!({}))
    }

    /// Handle tools/list request.
    fn handle_tools_list(&self, request: &McpRequest) -> McpResponse {
        let tools = self.registry.definitions();

        McpResponse::success(
            request.id.clone(),
            json!({
                "tools": tools
            }),
        )
    }

    /// Handle tools/call request.
    async fn handle_tools_call(&self, request: &McpRequest) -> McpResponse {
        // Extract tool name and arguments
        let name = request.params.get("name").and_then(|v| v.as_str());
        let arguments = request
            .params
            .get("arguments")
            .cloned()
            .unwrap_or(json!({}));

        let name = match name {
            Some(n) => n,
            None => {
                return McpResponse::error(
                    request.id.clone(),
                    error_codes::INVALID_PARAMS,
                    "missing tool name",
                );
            }
        };

        info!("Calling tool: {}", name);

        // Execute tool
        match self.registry.execute(name, arguments).await {
            Ok(result) => McpResponse::success(request.id.clone(), serde_json::to_value(result).unwrap()),
            Err(e) => {
                error!("Tool execution failed: {}", e);
                McpResponse::error(
                    request.id.clone(),
                    error_codes::INTERNAL_ERROR,
                    e.to_string(),
                )
            }
        }
    }

    /// Handle ping request.
    fn handle_ping(&self, request: &McpRequest) -> McpResponse {
        McpResponse::success(request.id.clone(), json!({}))
    }
}
