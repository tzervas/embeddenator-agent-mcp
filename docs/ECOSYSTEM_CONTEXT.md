# Embeddenator Agent MCP - Ecosystem Context Report

**Generated**: 2026-01-04  
**Purpose**: Cross-project context for AI-assisted development

## Project Identity

| Field | Value |
|-------|-------|
| **Name** | embeddenator-agent-mcp |
| **Type** | Native MCP Server (Rust binary) |
| **Transport** | stdio / HTTP |
| **Role** | Multi-agent orchestration across AI providers |

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
└───────────────┘ └───────────────┘ └───────────────┘
```

## MCP Tools Exposed

| Tool | Description |
|------|-------------|
| `agent_prompt` | Send prompt to best available provider |
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
| Claude | `claude` | 200k context, artifacts |
| ChatGPT | `chatgpt` | GPT-4o, vision, code |
| Gemini | `gemini` | 2M context |
| Grok | `grok` | Real-time info |
| Perplexity | `perplexity` | Search-focused |
| NotebookLM | `notebooklm` | Research assistant |

### Planned
- OpenAI API, Anthropic API, Google AI API
- Ollama, vLLM, LocalAI (self-hosted)

## Sister Projects

### Same Ecosystem (Native MCP Servers)

| Project | Relationship | Integration Point |
|---------|--------------|-------------------|
| **embeddenator-webpuppet-mcp** | Dependency | Provides browser automation backend |
| **embeddenator-context-mcp** | Sibling | Store workflow context between steps |
| **embeddenator-security-mcp** | Sibling | Screen prompts before sending to providers |

### WASM Execution Environment (homelab-ci-stack)

| Project | Relationship | Notes |
|---------|--------------|-------|
| **homelab-ci-stack/mcp-tool-coordinator** | Orchestrator | Alternative lightweight orchestration via WASM |
| **homelab-ci-stack/security-mcp** | Alternative | WASM-based input validation (~14ms) |
| **homelab-ci-stack/context-mcp** | Alternative | WASM-based text processing (~18ms) |

## Integration Patterns

### 1. Multi-Provider Consensus

```
agent_consensus(message, min_providers=3)
    │
    ├─► security-mcp.screen_input()
    │
    ├─► parallel:
    │   ├── webpuppet → claude
    │   ├── webpuppet → chatgpt
    │   └── webpuppet → gemini
    │
    └─► aggregate responses → consensus
```

### 2. Workflow with Context Persistence

```
agent_workflow_start(steps=[...])
    │
    ├─► Step 1: perplexity search
    │   └── context-mcp.store(result)
    │
    ├─► Step 2: claude summarize
    │   ├── context-mcp.retrieve()
    │   └── context-mcp.store(summary)
    │
    └─► Step 3: consensus verify
        └── context-mcp.retrieve()
```

### 3. homelab-ci-stack Hybrid (Future)

```
mcp-tool-coordinator
    │
    ├─► WASM pre-processing (sandboxed)
    │   ├── security-mcp.wasm → validate input
    │   └── context-mcp.wasm → extract context
    │
    └─► agent-mcp (container)
        └─► Full orchestration with state
```

## Key Dependencies

```toml
[dependencies]
embeddenator-webpuppet = { path = "../embeddenator-webpuppet" }
tokio = { version = "1.35", features = ["full", "sync", "time"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
clap = { version = "4.4", features = ["derive"] }
```

**Note**: Requires local `embeddenator-webpuppet` crate.

## Source Structure

```
src/
├── main.rs          # CLI entry, transport setup
├── lib.rs           # Public API
├── error.rs         # Error types
├── protocol.rs      # MCP JSON-RPC types
├── server.rs        # MCP server implementation
├── tools.rs         # Tool handlers (~20KB)
├── orchestrator.rs  # Multi-provider coordination (~15KB)
├── router.rs        # Provider routing logic (~10KB)
└── workflow.rs      # Multi-step workflow engine (~10KB)
```

## Testing & Validation

### Local Testing

```bash
# Run in stdio mode
cargo run -- --stdio --visible

# Test tool listing
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run -- --stdio
```

### With homelab-ci-stack Tools

The WASM security/context tools can pre-process before agent orchestration:

```bash
# Test security screening (WASM)
echo '{"method":"detect_injection","params":{"input":"test"}}' | \
  wasmtime run security-mcp.wasm
```

## CI/CD Context

### Current Workflows
- GitHub Actions for Rust CI
- Dependabot for dependency updates

### Recommended Additions
1. **Container image** for kubernetes deployment
2. **Integration tests** with webpuppet-mcp
3. **E2E tests** with multiple providers

## Development Notes

### State Management

The orchestrator maintains:
- Active workflows in memory
- Provider health/availability status
- Rate limiting counters per provider
- Session context for multi-turn

### WASM Compatibility

This server is **NOT WASM-compatible** because it requires:
- Long-running async workflows
- Network access to webpuppet/APIs
- Stateful session management

For sandboxed single-shot operations, use homelab-ci-stack WASM tools.

### Consensus Algorithm

1. Query N providers in parallel
2. Extract key claims from each response
3. Score agreement (semantic similarity)
4. Return highest-confidence answer with sources

---

*This report was generated to provide cross-project context for AI-assisted development across the embeddenator ecosystem.*
