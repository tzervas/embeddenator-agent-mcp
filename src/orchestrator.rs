//! Agent orchestrator for multi-provider prompt execution.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use embeddenator_webpuppet::{Provider, PromptRequest, PromptResponse, WebPuppet};

use crate::error::{Error, Result};
use crate::router::{ProviderRouter, TaskType};
use crate::workflow::{
    ProviderResponse, StepConfig, StepResult, StepState, Workflow, WorkflowState,
};

/// Orchestrator for multi-agent prompt execution.
pub struct AgentOrchestrator {
    /// WebPuppet instance for browser automation.
    puppet: Arc<RwLock<Option<WebPuppet>>>,
    /// Provider router for intelligent distribution.
    router: Arc<RwLock<ProviderRouter>>,
    /// Active workflows.
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
    /// Configuration.
    config: OrchestratorConfig,
}

impl AgentOrchestrator {
    /// Create a new orchestrator.
    pub fn new() -> Self {
        Self {
            puppet: Arc::new(RwLock::new(None)),
            router: Arc::new(RwLock::new(ProviderRouter::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            config: OrchestratorConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: OrchestratorConfig) -> Self {
        Self {
            puppet: Arc::new(RwLock::new(None)),
            router: Arc::new(RwLock::new(ProviderRouter::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get or create WebPuppet instance.
    async fn get_puppet(&self) -> Result<WebPuppet> {
        let guard = self.puppet.read().await;
        if guard.is_some() {
            drop(guard);
        }

        // Create new puppet
        let puppet = WebPuppet::builder()
            .with_all_providers()
            .headless(self.config.headless)
            .build()
            .await?;

        Ok(puppet)
    }

    /// Send a prompt to the best available provider.
    pub async fn prompt(&self, message: impl Into<String>) -> Result<PromptResponse> {
        let router = self.router.read().await;
        let provider = router.select_best(TaskType::General)?;
        drop(router);

        self.prompt_provider(provider, message).await
    }

    /// Send a prompt to a specific provider.
    pub async fn prompt_provider(
        &self,
        provider: Provider,
        message: impl Into<String>,
    ) -> Result<PromptResponse> {
        let message = message.into();
        let start = Instant::now();

        let puppet = self.get_puppet().await?;
        
        // Authenticate if needed
        puppet.authenticate(provider).await?;

        // Send prompt
        let request = PromptRequest::new(&message);
        let result = puppet.prompt(provider, request).await;

        // Record result in router
        let mut router = self.router.write().await;
        match &result {
            Ok(_) => router.record_success(provider, start.elapsed()),
            Err(_) => router.record_failure(provider),
        }

        // Cleanup
        puppet.close().await.ok();

        result.map_err(Error::from)
    }

    /// Send a prompt to multiple providers in parallel.
    ///
    /// Note: Due to browser automation constraints, this actually runs sequentially
    /// for web-based providers. API providers can run truly in parallel.
    pub async fn parallel_prompt(
        &self,
        message: impl Into<String>,
        providers: Vec<Provider>,
    ) -> Result<Vec<(Provider, Result<PromptResponse>)>> {
        let message = message.into();
        let puppet = self.get_puppet().await?;

        let mut results = Vec::new();
        
        // Run sequentially for browser-based providers
        // Future: API providers could run in parallel
        for provider in providers {
            // Authenticate
            let auth_result = puppet.authenticate(provider).await;
            if let Err(e) = auth_result {
                results.push((provider, Err(Error::from(e))));
                continue;
            }

            // Send prompt
            let request = PromptRequest::new(&message);
            let prompt_result = puppet.prompt(provider, request).await;
            
            results.push((provider, prompt_result.map_err(Error::from)));
        }

        puppet.close().await.ok();

        Ok(results)
    }

    /// Get consensus from multiple providers.
    pub async fn consensus_prompt(
        &self,
        message: impl Into<String>,
        min_providers: usize,
    ) -> Result<ConsensusResult> {
        let message = message.into();
        
        // Select providers
        let router = self.router.read().await;
        let providers = router.select_multiple(min_providers.max(3), TaskType::General)?;
        drop(router);

        // Get responses in parallel
        let results = self.parallel_prompt(&message, providers).await?;

        // Collect successful responses
        let responses: Vec<_> = results
            .into_iter()
            .filter_map(|(p, r)| r.ok().map(|resp| (p, resp)))
            .collect();

        if responses.len() < min_providers {
            return Err(Error::NoProviders(format!(
                "only {} providers responded, need {}",
                responses.len(),
                min_providers
            )));
        }

        // Simple consensus: find common themes
        // In a real implementation, this would use semantic similarity
        let consensus = self.find_consensus(&responses);

        Ok(consensus)
    }

    /// Find consensus among responses (simple implementation).
    fn find_consensus(&self, responses: &[(Provider, PromptResponse)]) -> ConsensusResult {
        // For now, just return the longest response as "consensus"
        // A real implementation would use semantic similarity
        let best = responses
            .iter()
            .max_by_key(|(_, r)| r.text.len())
            .map(|(p, r)| (*p, r.clone()));

        let provider_responses: Vec<_> = responses
            .iter()
            .map(|(p, r)| ProviderResponse {
                provider: p.to_string(),
                text: r.text.clone(),
                selected: best.as_ref().map_or(false, |(bp, _)| bp == p),
                confidence: None,
            })
            .collect();

        ConsensusResult {
            consensus_text: best.map(|(_, r)| r.text).unwrap_or_default(),
            responses: provider_responses,
            agreement_score: 0.5, // Placeholder
        }
    }

    /// Start a new workflow.
    pub async fn start_workflow(&self, workflow: Workflow) -> Result<String> {
        let id = workflow.id.clone();
        let mut workflows = self.workflows.write().await;
        workflows.insert(id.clone(), workflow);
        Ok(id)
    }

    /// Execute the next step in a workflow.
    pub async fn execute_workflow_step(&self, workflow_id: &str) -> Result<StepResult> {
        let mut workflows = self.workflows.write().await;
        let workflow = workflows
            .get_mut(workflow_id)
            .ok_or_else(|| Error::Workflow(format!("workflow not found: {}", workflow_id)))?;

        if workflow.is_complete() {
            return Err(Error::InvalidState("workflow already complete".into()));
        }

        // Get step config (clone to avoid borrow issues)
        let step_config = workflow
            .current()
            .ok_or_else(|| Error::InvalidState("no current step".into()))?
            .config
            .clone();

        // Mark step as running
        if let Some(step) = workflow.current_mut() {
            step.start();
        }
        workflow.state = WorkflowState::Running;

        let start = Instant::now();
        let result = match &step_config {
            StepConfig::Prompt { message, provider, context } => {
                let provider = provider
                    .as_ref()
                    .and_then(|p| match p.to_lowercase().as_str() {
                        "claude" => Some(Provider::Claude),
                        "grok" => Some(Provider::Grok),
                        "gemini" => Some(Provider::Gemini),
                        "chatgpt" => Some(Provider::ChatGpt),
                        "perplexity" => Some(Provider::Perplexity),
                        "notebooklm" => Some(Provider::NotebookLm),
                        _ => None,
                    });

                // Note: context is currently not used in prompt_provider
                // Future: pass context as system message
                let _context_for_future = context;

                let response = if let Some(p) = provider {
                    self.prompt_provider(p, message.clone()).await?
                } else {
                    self.prompt(message.clone()).await?
                };

                StepResult {
                    output: response.text,
                    provider: Some(response.provider.to_string()),
                    responses: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    metadata: HashMap::new(),
                }
            }
            StepConfig::ParallelPrompt { message, providers } => {
                let providers: Vec<_> = providers
                    .iter()
                    .filter_map(|p| match p.to_lowercase().as_str() {
                        "claude" => Some(Provider::Claude),
                        "grok" => Some(Provider::Grok),
                        "gemini" => Some(Provider::Gemini),
                        "chatgpt" => Some(Provider::ChatGpt),
                        "perplexity" => Some(Provider::Perplexity),
                        "notebooklm" => Some(Provider::NotebookLm),
                        _ => None,
                    })
                    .collect();

                let results = self.parallel_prompt(message.clone(), providers).await?;
                
                let responses: Vec<_> = results
                    .iter()
                    .filter_map(|(p, r)| {
                        r.as_ref().ok().map(|resp| ProviderResponse {
                            provider: p.to_string(),
                            text: resp.text.clone(),
                            selected: false,
                            confidence: None,
                        })
                    })
                    .collect();

                let output = responses
                    .iter()
                    .map(|r| format!("**{}**:\n{}", r.provider, r.text))
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n");

                StepResult {
                    output,
                    provider: None,
                    responses: Some(responses),
                    duration_ms: start.elapsed().as_millis() as u64,
                    metadata: HashMap::new(),
                }
            }
            StepConfig::Consensus { message, min_providers } => {
                let consensus = self.consensus_prompt(message.clone(), *min_providers).await?;

                StepResult {
                    output: consensus.consensus_text,
                    provider: None,
                    responses: Some(consensus.responses),
                    duration_ms: start.elapsed().as_millis() as u64,
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert(
                            "agreement_score".into(),
                            serde_json::json!(consensus.agreement_score),
                        );
                        m
                    },
                }
            }
            StepConfig::HumanReview { prompt: _ } => {
                // Set step to waiting and return
                let step = workflow.current_mut().unwrap();
                step.state = StepState::WaitingForHuman;
                workflow.state = WorkflowState::Paused;
                
                return Err(Error::Workflow("waiting for human review".into()));
            }
            _ => {
                return Err(Error::Workflow("unsupported step type".into()));
            }
        };

        // Mark step complete and advance
        let step = workflow.current_mut().unwrap();
        step.complete(result.clone());
        workflow.advance()?;

        Ok(result)
    }

    /// Get a workflow by ID.
    pub async fn get_workflow(&self, id: &str) -> Option<Workflow> {
        let workflows = self.workflows.read().await;
        workflows.get(id).cloned()
    }

    /// Get orchestrator status.
    pub async fn status(&self) -> OrchestratorStatus {
        let router = self.router.read().await;
        let workflows = self.workflows.read().await;

        OrchestratorStatus {
            available_providers: router.available_providers(),
            active_workflows: workflows.len(),
            provider_stats: router.get_stats(),
        }
    }
}

impl Default for AgentOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AgentOrchestrator {
    fn clone(&self) -> Self {
        Self {
            puppet: self.puppet.clone(),
            router: self.router.clone(),
            workflows: self.workflows.clone(),
            config: self.config.clone(),
        }
    }
}

/// Orchestrator configuration.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Run browsers in headless mode.
    pub headless: bool,
    /// Default timeout for operations.
    pub timeout: Duration,
    /// Maximum concurrent requests.
    pub max_concurrent: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            headless: true,
            timeout: Duration::from_secs(120),
            max_concurrent: 5,
        }
    }
}

/// Result of a consensus operation.
#[derive(Debug, Clone)]
pub struct ConsensusResult {
    /// The consensus text.
    pub consensus_text: String,
    /// All provider responses.
    pub responses: Vec<ProviderResponse>,
    /// Agreement score (0.0 - 1.0).
    pub agreement_score: f64,
}

/// Orchestrator status.
#[derive(Debug, Clone)]
pub struct OrchestratorStatus {
    /// Available providers.
    pub available_providers: Vec<Provider>,
    /// Number of active workflows.
    pub active_workflows: usize,
    /// Provider statistics.
    pub provider_stats: HashMap<Provider, crate::router::ProviderStats>,
}
