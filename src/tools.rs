//! Tool definitions for agent-mcp.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

use embeddenator_webpuppet::Provider;

use crate::error::{Error, Result};
use crate::orchestrator::AgentOrchestrator;
use crate::protocol::{ContentItem, ToolCallResult, ToolDefinition};
use crate::workflow::{Workflow, WorkflowStep};

/// Tool trait for implementing MCP tools.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool definition.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments.
    async fn execute(
        &self,
        arguments: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolCallResult>;
}

/// Context passed to tools during execution.
pub struct ToolContext {
    /// Agent orchestrator.
    pub orchestrator: Arc<AgentOrchestrator>,
    /// Whether to show browser (non-headless).
    pub visible: bool,
}

impl ToolContext {
    /// Create a new tool context.
    pub fn new(orchestrator: AgentOrchestrator) -> Self {
        Self {
            orchestrator: Arc::new(orchestrator),
            visible: false,
        }
    }

    /// Create a tool context with visible browser.
    pub fn with_visible_browser(orchestrator: AgentOrchestrator) -> Self {
        Self {
            orchestrator: Arc::new(orchestrator),
            visible: true,
        }
    }
}

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    context: Arc<ToolContext>,
}

impl ToolRegistry {
    /// Create a new tool registry with default tools.
    pub fn new(orchestrator: AgentOrchestrator) -> Self {
        Self::with_context(ToolContext::new(orchestrator))
    }

    /// Create a tool registry with custom context.
    pub fn with_context(context: ToolContext) -> Self {
        let context = Arc::new(context);
        let mut registry = Self {
            tools: HashMap::new(),
            context,
        };
        registry.register_default_tools();
        registry
    }

    /// Register default tools.
    fn register_default_tools(&mut self) {
        self.register(Arc::new(PromptTool));
        self.register(Arc::new(ParallelPromptTool));
        self.register(Arc::new(ConsensusTool));
        self.register(Arc::new(WorkflowStartTool));
        self.register(Arc::new(WorkflowStepTool));
        self.register(Arc::new(StatusTool));
        self.register(Arc::new(ListProvidersTool));
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.definition().name.clone();
        self.tools.insert(name, tool);
    }

    /// Get all tool definitions.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name.
    pub async fn execute(&self, name: &str, arguments: serde_json::Value) -> Result<ToolCallResult> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| Error::InvalidParams(format!("unknown tool: {}", name)))?;

        tool.execute(arguments, &self.context).await
    }
}

// =============================================================================
// Tool Implementations
// =============================================================================

/// Tool for sending a prompt to the best available provider.
pub struct PromptTool;

#[derive(Debug, Deserialize)]
struct PromptArgs {
    message: String,
    provider: Option<String>,
    context: Option<String>,
}

#[async_trait::async_trait]
impl Tool for PromptTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "agent_prompt".into(),
            description: "Send a prompt to an AI provider. If no provider specified, uses the best available.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The prompt message to send"
                    },
                    "provider": {
                        "type": "string",
                        "enum": ["claude", "grok", "gemini", "chatgpt", "perplexity", "notebooklm"],
                        "description": "Optional: specific provider to use"
                    },
                    "context": {
                        "type": "string",
                        "description": "Optional: system context or instructions"
                    }
                },
                "required": ["message"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolCallResult> {
        let args: PromptArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidParams(e.to_string()))?;

        let response = if let Some(provider_str) = args.provider {
            let provider = parse_provider(&provider_str)?;
            context.orchestrator.prompt_provider(provider, args.message).await?
        } else {
            context.orchestrator.prompt(args.message).await?
        };

        Ok(ToolCallResult {
            content: vec![ContentItem::text(format!(
                "**Response from {}:**\n\n{}",
                response.provider, response.text
            ))],
            is_error: false,
        })
    }
}

/// Tool for sending a prompt to multiple providers in parallel.
pub struct ParallelPromptTool;

#[derive(Debug, Deserialize)]
struct ParallelPromptArgs {
    message: String,
    providers: Vec<String>,
}

#[async_trait::async_trait]
impl Tool for ParallelPromptTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "agent_parallel_prompt".into(),
            description: "Send the same prompt to multiple AI providers in parallel.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The prompt message to send"
                    },
                    "providers": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["claude", "grok", "gemini", "chatgpt", "perplexity", "notebooklm"]
                        },
                        "description": "List of providers to query",
                        "minItems": 2
                    }
                },
                "required": ["message", "providers"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolCallResult> {
        let args: ParallelPromptArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidParams(e.to_string()))?;

        let providers: Vec<Provider> = args
            .providers
            .iter()
            .filter_map(|p| parse_provider(p).ok())
            .collect();

        if providers.len() < 2 {
            return Err(Error::InvalidParams("need at least 2 valid providers".into()));
        }

        let results = context
            .orchestrator
            .parallel_prompt(args.message, providers)
            .await?;

        let text = results
            .iter()
            .map(|(provider, result)| match result {
                Ok(resp) => format!("## {}\n\n{}", provider, resp.text),
                Err(e) => format!("## {} (Error)\n\n{}", provider, e),
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        Ok(ToolCallResult {
            content: vec![ContentItem::text(format!(
                "# Parallel Responses\n\n{}",
                text
            ))],
            is_error: false,
        })
    }
}

/// Tool for getting consensus from multiple providers.
pub struct ConsensusTool;

#[derive(Debug, Deserialize)]
struct ConsensusArgs {
    message: String,
    min_providers: Option<usize>,
}

#[async_trait::async_trait]
impl Tool for ConsensusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "agent_consensus".into(),
            description: "Get a consensus answer from multiple AI providers.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The question to get consensus on"
                    },
                    "min_providers": {
                        "type": "integer",
                        "description": "Minimum providers to query (default: 3)",
                        "minimum": 2,
                        "default": 3
                    }
                },
                "required": ["message"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolCallResult> {
        let args: ConsensusArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidParams(e.to_string()))?;

        let min_providers = args.min_providers.unwrap_or(3);

        let result = context
            .orchestrator
            .consensus_prompt(args.message, min_providers)
            .await?;

        let responses_text = result
            .responses
            .iter()
            .map(|r| {
                let marker = if r.selected { "✓" } else { "○" };
                format!("{} **{}**: {}", marker, r.provider, r.text.chars().take(200).collect::<String>())
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(ToolCallResult {
            content: vec![ContentItem::text(format!(
                "# Consensus Result\n\n**Agreement Score:** {:.0}%\n\n## Consensus Answer\n\n{}\n\n## Individual Responses\n\n{}",
                result.agreement_score * 100.0,
                result.consensus_text,
                responses_text
            ))],
            is_error: false,
        })
    }
}

/// Tool for starting a new workflow.
pub struct WorkflowStartTool;

#[derive(Debug, Deserialize)]
struct WorkflowStartArgs {
    name: String,
    steps: Vec<WorkflowStepDef>,
}

#[derive(Debug, Deserialize)]
struct WorkflowStepDef {
    name: String,
    #[serde(rename = "type")]
    step_type: String,
    message: String,
    provider: Option<String>,
    providers: Option<Vec<String>>,
}

#[async_trait::async_trait]
impl Tool for WorkflowStartTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "agent_workflow_start".into(),
            description: "Start a new multi-step workflow.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the workflow"
                    },
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "type": {
                                    "type": "string",
                                    "enum": ["prompt", "parallel", "consensus", "review"]
                                },
                                "message": { "type": "string" },
                                "provider": { "type": "string" },
                                "providers": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                }
                            },
                            "required": ["name", "type", "message"]
                        },
                        "description": "Workflow steps to execute"
                    }
                },
                "required": ["name", "steps"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolCallResult> {
        let args: WorkflowStartArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidParams(e.to_string()))?;

        let mut workflow = Workflow::new(args.name);

        for step_def in args.steps {
            let step = match step_def.step_type.as_str() {
                "prompt" => WorkflowStep::prompt(step_def.name, step_def.message),
                "parallel" => WorkflowStep::parallel(
                    step_def.name,
                    step_def.message,
                    step_def.providers.unwrap_or_default(),
                ),
                "consensus" => WorkflowStep::consensus(step_def.name, step_def.message),
                "review" => WorkflowStep::review(step_def.name, step_def.message),
                _ => return Err(Error::InvalidParams(format!("unknown step type: {}", step_def.step_type))),
            };
            workflow.add_step(step);
        }

        let id = context.orchestrator.start_workflow(workflow).await?;

        Ok(ToolCallResult {
            content: vec![ContentItem::text(format!(
                "# Workflow Started\n\n**ID:** `{}`\n\nUse `agent_workflow_step` with this ID to execute steps.",
                id
            ))],
            is_error: false,
        })
    }
}

/// Tool for executing the next step in a workflow.
pub struct WorkflowStepTool;

#[derive(Debug, Deserialize)]
struct WorkflowStepArgs {
    workflow_id: String,
}

#[async_trait::async_trait]
impl Tool for WorkflowStepTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "agent_workflow_step".into(),
            description: "Execute the next step in a workflow.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workflow_id": {
                        "type": "string",
                        "description": "ID of the workflow to execute"
                    }
                },
                "required": ["workflow_id"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolCallResult> {
        let args: WorkflowStepArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidParams(e.to_string()))?;

        let result = context
            .orchestrator
            .execute_workflow_step(&args.workflow_id)
            .await?;

        let workflow = context
            .orchestrator
            .get_workflow(&args.workflow_id)
            .await
            .ok_or_else(|| Error::Workflow("workflow not found".into()))?;

        let status = if workflow.is_complete() {
            "✅ Workflow Complete"
        } else {
            &format!("Step {}/{}", workflow.current_step, workflow.steps.len())
        };

        Ok(ToolCallResult {
            content: vec![ContentItem::text(format!(
                "# Workflow Step Result\n\n**Status:** {}\n**Duration:** {}ms\n\n## Output\n\n{}",
                status, result.duration_ms, result.output
            ))],
            is_error: false,
        })
    }
}

/// Tool for getting orchestrator status.
pub struct StatusTool;

#[async_trait::async_trait]
impl Tool for StatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "agent_status".into(),
            description: "Get the status of the agent orchestrator.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(
        &self,
        _arguments: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolCallResult> {
        let status = context.orchestrator.status().await;

        let providers_text = status
            .available_providers
            .iter()
            .map(|p| format!("- ✅ {}", p))
            .collect::<Vec<_>>()
            .join("\n");

        let stats_text = status
            .provider_stats
            .iter()
            .map(|(p, s)| {
                format!(
                    "- **{}**: {} total, {} success, {} failed",
                    p, s.total_requests, s.successful_requests, s.failed_requests
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolCallResult {
            content: vec![ContentItem::text(format!(
                "# Agent Orchestrator Status\n\n## Available Providers\n\n{}\n\n## Active Workflows\n\n{}\n\n## Provider Statistics\n\n{}",
                providers_text,
                status.active_workflows,
                if stats_text.is_empty() { "No requests yet".into() } else { stats_text }
            ))],
            is_error: false,
        })
    }
}

/// Tool for listing available providers.
pub struct ListProvidersTool;

#[async_trait::async_trait]
impl Tool for ListProvidersTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "agent_list_providers".into(),
            description: "List all available AI providers and their capabilities.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(
        &self,
        _arguments: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolCallResult> {
        let providers = vec![
            ("claude", "Claude (Anthropic)", "200k context, artifacts, code execution"),
            ("grok", "Grok (X/xAI)", "Real-time info, X integration"),
            ("gemini", "Gemini (Google)", "2M context, Google integration"),
            ("chatgpt", "ChatGPT (OpenAI)", "GPT-4o, vision, web search, code"),
            ("perplexity", "Perplexity AI", "Search-focused, sources cited"),
            ("notebooklm", "NotebookLM (Google)", "500k context, research assistant"),
        ];

        let text = providers
            .iter()
            .map(|(id, name, caps)| format!("## {} (`{}`)\n\n{}\n", name, id, caps))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolCallResult {
            content: vec![ContentItem::text(format!(
                "# Available AI Providers\n\n{}",
                text
            ))],
            is_error: false,
        })
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Parse provider string to Provider enum.
fn parse_provider(s: &str) -> Result<Provider> {
    match s.to_lowercase().as_str() {
        "claude" => Ok(Provider::Claude),
        "grok" => Ok(Provider::Grok),
        "gemini" => Ok(Provider::Gemini),
        "chatgpt" | "openai" => Ok(Provider::ChatGpt),
        "perplexity" => Ok(Provider::Perplexity),
        "notebooklm" | "notebook" => Ok(Provider::NotebookLm),
        _ => Err(Error::InvalidParams(format!("unknown provider: {}", s))),
    }
}
