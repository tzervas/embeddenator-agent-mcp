//! Agent MCP Server - Multi-agent orchestration for VS Code/GitHub Copilot.

use clap::Parser;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use embeddenator_agent_mcp::{AgentMcpServer, AgentOrchestrator};

/// Agent MCP Server - Multi-agent orchestration for AI providers.
#[derive(Parser, Debug)]
#[command(name = "agent-mcp")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run browser in visible (non-headless) mode.
    #[arg(long, default_value = "false")]
    visible: bool,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Output logs as JSON.
    #[arg(long, default_value = "false")]
    json_logs: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize logging - output to stderr to avoid interfering with MCP protocol
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&args.log_level));

    if args.json_logs {
        fmt()
            .json()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    } else {
        fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    }

    info!("Agent MCP Server starting");
    info!("Visible mode: {}", args.visible);

    // Create orchestrator with configuration
    let config = embeddenator_agent_mcp::orchestrator::OrchestratorConfig {
        headless: !args.visible,
        ..Default::default()
    };
    let orchestrator = AgentOrchestrator::with_config(config);

    // Create and run server
    let mut server = AgentMcpServer::new(orchestrator);
    server.run_stdio().await?;

    Ok(())
}
