//! Workflow management for multi-step agent tasks.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

/// A workflow represents a multi-step agent task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Unique workflow ID.
    pub id: String,
    /// Workflow name/description.
    pub name: String,
    /// Current state.
    pub state: WorkflowState,
    /// Workflow steps.
    pub steps: Vec<WorkflowStep>,
    /// Current step index.
    pub current_step: usize,
    /// Context passed between steps.
    pub context: HashMap<String, serde_json::Value>,
    /// When the workflow was created.
    pub created_at: DateTime<Utc>,
    /// When the workflow was last updated.
    pub updated_at: DateTime<Utc>,
    /// Workflow metadata.
    pub metadata: HashMap<String, String>,
}

impl Workflow {
    /// Create a new workflow.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            state: WorkflowState::Pending,
            steps: Vec::new(),
            current_step: 0,
            context: HashMap::new(),
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Add a step to the workflow.
    pub fn add_step(&mut self, step: WorkflowStep) {
        self.steps.push(step);
        self.updated_at = Utc::now();
    }

    /// Get the current step.
    pub fn current(&self) -> Option<&WorkflowStep> {
        self.steps.get(self.current_step)
    }

    /// Get the current step mutably.
    pub fn current_mut(&mut self) -> Option<&mut WorkflowStep> {
        self.steps.get_mut(self.current_step)
    }

    /// Advance to the next step.
    pub fn advance(&mut self) -> Result<()> {
        if self.current_step >= self.steps.len() {
            return Err(Error::InvalidState("workflow already complete".into()));
        }
        self.current_step += 1;
        self.updated_at = Utc::now();
        
        if self.current_step >= self.steps.len() {
            self.state = WorkflowState::Completed;
        }
        Ok(())
    }

    /// Set workflow to failed state.
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.state = WorkflowState::Failed(reason.into());
        self.updated_at = Utc::now();
    }

    /// Check if workflow is complete.
    pub fn is_complete(&self) -> bool {
        matches!(self.state, WorkflowState::Completed | WorkflowState::Failed(_))
    }

    /// Set context value.
    pub fn set_context(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.context.insert(key.into(), value);
        self.updated_at = Utc::now();
    }

    /// Get context value.
    pub fn get_context(&self, key: &str) -> Option<&serde_json::Value> {
        self.context.get(key)
    }
}

/// State of a workflow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status")]
pub enum WorkflowState {
    /// Workflow is pending, not yet started.
    #[serde(rename = "pending")]
    Pending,
    /// Workflow is running.
    #[serde(rename = "running")]
    Running,
    /// Workflow is paused, waiting for input.
    #[serde(rename = "paused")]
    Paused,
    /// Workflow completed successfully.
    #[serde(rename = "completed")]
    Completed,
    /// Workflow failed.
    #[serde(rename = "failed")]
    Failed(String),
}

/// A single step in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Step ID.
    pub id: String,
    /// Step name/description.
    pub name: String,
    /// Step type.
    pub step_type: StepType,
    /// Step state.
    pub state: StepState,
    /// Step configuration.
    pub config: StepConfig,
    /// Result of the step (if completed).
    pub result: Option<StepResult>,
}

impl WorkflowStep {
    /// Create a new prompt step.
    pub fn prompt(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            step_type: StepType::Prompt,
            state: StepState::Pending,
            config: StepConfig::Prompt {
                message: message.into(),
                provider: None,
                context: None,
            },
            result: None,
        }
    }

    /// Create a parallel prompt step.
    pub fn parallel(name: impl Into<String>, message: impl Into<String>, providers: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            step_type: StepType::ParallelPrompt,
            state: StepState::Pending,
            config: StepConfig::ParallelPrompt {
                message: message.into(),
                providers,
            },
            result: None,
        }
    }

    /// Create a consensus step.
    pub fn consensus(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            step_type: StepType::Consensus,
            state: StepState::Pending,
            config: StepConfig::Consensus {
                message: message.into(),
                min_providers: 2,
            },
            result: None,
        }
    }

    /// Create a human review step.
    pub fn review(name: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            step_type: StepType::HumanReview,
            state: StepState::Pending,
            config: StepConfig::HumanReview {
                prompt: prompt.into(),
            },
            result: None,
        }
    }

    /// Mark step as running.
    pub fn start(&mut self) {
        self.state = StepState::Running;
    }

    /// Mark step as completed with result.
    pub fn complete(&mut self, result: StepResult) {
        self.state = StepState::Completed;
        self.result = Some(result);
    }

    /// Mark step as failed.
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.state = StepState::Failed(reason.into());
    }
}

/// Type of workflow step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepType {
    /// Send a prompt to a single provider.
    Prompt,
    /// Send a prompt to multiple providers in parallel.
    ParallelPrompt,
    /// Get consensus from multiple providers.
    Consensus,
    /// Require human review/approval.
    HumanReview,
    /// Conditional branching.
    Conditional,
    /// Custom tool invocation.
    Tool,
}

/// State of a workflow step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepState {
    /// Step is pending.
    Pending,
    /// Step is running.
    Running,
    /// Step is waiting for human input.
    WaitingForHuman,
    /// Step completed successfully.
    Completed,
    /// Step failed.
    Failed(String),
}

/// Configuration for a workflow step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepConfig {
    /// Prompt configuration.
    #[serde(rename = "prompt")]
    Prompt {
        message: String,
        provider: Option<String>,
        context: Option<String>,
    },
    /// Parallel prompt configuration.
    #[serde(rename = "parallel")]
    ParallelPrompt {
        message: String,
        providers: Vec<String>,
    },
    /// Consensus configuration.
    #[serde(rename = "consensus")]
    Consensus {
        message: String,
        min_providers: usize,
    },
    /// Human review configuration.
    #[serde(rename = "human_review")]
    HumanReview {
        prompt: String,
    },
    /// Conditional configuration.
    #[serde(rename = "conditional")]
    Conditional {
        condition: String,
        then_step: String,
        else_step: Option<String>,
    },
    /// Tool invocation configuration.
    #[serde(rename = "tool")]
    Tool {
        tool_name: String,
        arguments: serde_json::Value,
    },
}

/// Result of a workflow step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Output text/content.
    pub output: String,
    /// Provider that generated the response.
    pub provider: Option<String>,
    /// Multiple responses (for parallel/consensus steps).
    pub responses: Option<Vec<ProviderResponse>>,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
    /// Additional metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Response from a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// Provider name.
    pub provider: String,
    /// Response text.
    pub text: String,
    /// Whether this was selected as the consensus answer.
    pub selected: bool,
    /// Confidence/agreement score (0.0-1.0).
    pub confidence: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_creation() {
        let mut workflow = Workflow::new("test workflow");
        assert_eq!(workflow.state, WorkflowState::Pending);
        assert_eq!(workflow.steps.len(), 0);

        workflow.add_step(WorkflowStep::prompt("step 1", "Hello"));
        assert_eq!(workflow.steps.len(), 1);
    }

    #[test]
    fn test_workflow_advance() {
        let mut workflow = Workflow::new("test");
        workflow.add_step(WorkflowStep::prompt("step 1", "Hello"));
        workflow.add_step(WorkflowStep::prompt("step 2", "World"));

        assert_eq!(workflow.current_step, 0);
        workflow.advance().unwrap();
        assert_eq!(workflow.current_step, 1);
        workflow.advance().unwrap();
        assert!(workflow.is_complete());
    }
}
