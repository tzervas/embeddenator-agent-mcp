//! Multi-Agent Orchestration MCP Server
//!
//! This crate provides an MCP server for orchestrating multi-agent workflows
//! in VS Code and GitHub Copilot environments. It enables:
//!
//! - Automated prompt routing to multiple AI providers
//! - Secure workflow execution with human-in-the-loop controls
//! - Sub-agent delegation to webpuppet for web-based AI interactions
//! - Workflow state management and persistence
//! - Rate limiting and cost tracking across providers
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    VS Code / GitHub Copilot                      │
//! └───────────────────────────┬─────────────────────────────────────┘
//!                             │ MCP Protocol (JSON-RPC over stdio)
//!                             ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   embeddenator-agent-mcp                         │
//! │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐   │
//! │  │ Workflow   │ │ Provider   │ │ Security   │ │ Session    │   │
//! │  │ Manager    │ │ Router     │ │ Guard      │ │ Manager    │   │
//! │  └────────────┘ └────────────┘ └────────────┘ └────────────┘   │
//! └───────────────────────────┬─────────────────────────────────────┘
//!                             │
//!         ┌───────────────────┼───────────────────┐
//!         ▼                   ▼                   ▼
//! ┌───────────────┐ ┌───────────────┐ ┌───────────────┐
//! │  webpuppet    │ │   API         │ │  Self-hosted  │
//! │  (browser)    │ │   Providers   │ │  (future)     │
//! │               │ │   (future)    │ │               │
//! │ Claude, Grok  │ │ OpenAI API    │ │ Ollama        │
//! │ Gemini, etc.  │ │ Anthropic API │ │ vLLM          │
//! └───────────────┘ └───────────────┘ └───────────────┘
//! ```
//!
//! # MCP Tools
//!
//! | Tool | Description |
//! |------|-------------|
//! | `agent_prompt` | Send a prompt to best available provider |
//! | `agent_workflow_start` | Start a multi-step workflow |
//! | `agent_workflow_step` | Execute next step in workflow |
//! | `agent_parallel_prompt` | Send same prompt to multiple providers |
//! | `agent_consensus` | Get consensus answer from multiple providers |
//! | `agent_status` | Get orchestration status and stats |
//! | `agent_config` | Configure provider preferences |

pub mod error;
pub mod orchestrator;
pub mod protocol;
pub mod router;
pub mod server;
pub mod tools;
pub mod workflow;

pub use error::{Error, Result};
pub use orchestrator::AgentOrchestrator;
pub use protocol::{McpRequest, McpResponse};
pub use router::ProviderRouter;
pub use server::AgentMcpServer;
pub use workflow::{Workflow, WorkflowStep, WorkflowState};
