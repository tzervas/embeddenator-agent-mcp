# embeddenator-agent-mcp

Multi-agent orchestration MCP server for VS Code and GitHub Copilot.

## Overview

`embeddenator-agent-mcp` provides a Model Context Protocol (MCP) server for orchestrating prompts across multiple AI providers. It enables:

- **Intelligent Provider Routing**: Automatically select the best provider based on task type
- **Parallel Execution**: Query multiple providers simultaneously  
- **Consensus Building**: Get agreement from multiple AI providers
- **Workflow Management**: Define multi-step automation workflows
- **Security**: Content screening and rate limiting built-in

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    VS Code / GitHub Copilot                      │
└───────────────────────────┬─────────────────────────────────────┘
                            │ MCP Protocol (JSON-RPC over stdio)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                   embeddenator-agent-mcp                         │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐   │
│  │ Workflow   │ │ Provider   │ │ Security   │ │ Session    │   │
│  │ Manager    │ │ Router     │ │ Guard      │ │ Manager    │   │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘   │
└───────────────────────────┬─────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│  webpuppet    │ │   API         │ │  Self-hosted  │
│  (browser)    │ │   Providers   │ │  (future)     │
│               │ │   (future)    │ │               │
│ Claude, Grok  │ │ OpenAI API    │ │ Ollama        │
│ Gemini, etc.  │ │ Anthropic API │ │ vLLM          │
└───────────────┘ └───────────────┘ └───────────────┘
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `agent_prompt` | Send a prompt to best available provider |
| `agent_parallel_prompt` | Send same prompt to multiple providers |
| `agent_consensus` | Get consensus answer from multiple providers |
| `agent_workflow_start` | Start a multi-step workflow |
| `agent_workflow_step` | Execute next step in workflow |
| `agent_status` | Get orchestration status and stats |
| `agent_list_providers` | List available AI providers |

## Supported Providers

### Web-based (via webpuppet)

| Provider | ID | Features |
|----------|-----|----------|
| Claude (Anthropic) | `claude` | 200k context, artifacts, code execution |
| ChatGPT (OpenAI) | `chatgpt` | GPT-4o, vision, web search, code |
| Gemini (Google) | `gemini` | 2M context, Google integration |
| Grok (X/xAI) | `grok` | Real-time info, X integration |
| Perplexity AI | `perplexity` | Search-focused, sources cited |
| NotebookLM | `notebooklm` | 500k context, research assistant |
| Kaggle (Datasets) | `kaggle` | Dataset search/catalog (links + metadata) |

### API-based (planned)

- OpenAI API
- Anthropic API  
- Google AI API

### Self-hosted (planned)

- Ollama
- vLLM
- text-generation-webui
- LocalAI

## Installation

### Building from Source

```bash
cargo build -p embeddenator-agent-mcp --release
```

### VS Code Integration

Add to your VS Code `mcp.json`:

```json
{
  "servers": {
    "agent": {
      "command": "/path/to/agent-mcp",
      "args": ["--visible"]
    }
  }
}
```

## Usage

### Basic Prompt

```json
{
  "name": "agent_prompt",
  "arguments": {
    "message": "Explain quantum computing in simple terms"
  }
}
```

### Parallel Prompt

```json
{
  "name": "agent_parallel_prompt", 
  "arguments": {
    "message": "What are the best practices for API design?",
    "providers": ["claude", "chatgpt", "gemini"]
  }
}
```

### Consensus

```json
{
  "name": "agent_consensus",
  "arguments": {
    "message": "What is the capital of France?",
    "min_providers": 3
  }
}
```

### Workflow

```json
{
  "name": "agent_workflow_start",
  "arguments": {
    "name": "Research Workflow",
    "steps": [
      {
        "name": "Search",
        "type": "prompt",
        "message": "Find recent papers on quantum computing",
        "provider": "perplexity"
      },
      {
        "name": "Summarize", 
        "type": "prompt",
        "message": "Summarize the key findings",
        "provider": "claude"
      },
      {
        "name": "Verify",
        "type": "consensus",
        "message": "Verify the accuracy of this summary"
      }
    ]
  }
}
```

## CLI Options

```
agent-mcp [OPTIONS]

Options:
  --visible         Run browser in visible (non-headless) mode
  --log-level       Log level (trace, debug, info, warn, error) [default: info]
  --json-logs       Output logs as JSON
  -h, --help        Print help
  -V, --version     Print version
```

## License

MIT
