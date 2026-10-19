//! Scripts (macros): a list of commands with their arguments, as a JSON
//! array of `{"command": ..., "args": {...}}` objects or one such object per
//! line (`//` lines are comments). `ccad script` runs them, the app records
//! and plays them as macros.

use crate::error::{EngineError, Result};
use crate::session::Session;
use serde_json::{Value, json};

/// One step of a script.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub command: String,
    pub args: Value,
}

impl Step {
    pub fn to_json(&self) -> Value {
        json!({ "command": self.command, "args": self.args })
    }
}

/// Read a script.
pub fn parse(text: &str) -> Result<Vec<Step>> {
    let items: Vec<Value> = if text.trim_start().starts_with('[') {
        serde_json::from_str(text).map_err(|e| EngineError::invalid(format!("invalid script: {e}")))?
    } else {
        text.lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
            .map(|(i, l)| {
                serde_json::from_str(l)
                    .map_err(|e| EngineError::invalid(format!("invalid script, line {}: {e}", i + 1)))
            })
            .collect::<Result<_>>()?
    };
    items
        .into_iter()
        .map(|item| {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .ok_or_else(|| EngineError::invalid("each step needs a \"command\""))?;
            Ok(Step { command: command.to_string(), args: item.get("args").cloned().unwrap_or(json!({})) })
        })
        .collect()
}

/// A script with one step per line (the format macros are saved in).
pub fn to_text(steps: &[Step]) -> String {
    steps.iter().map(|s| format!("{}\n", s.to_json())).collect()
}

/// A failed step: its index (from 0) and the error.
#[derive(Debug, Clone, PartialEq)]
pub struct StepError {
    pub index: usize,
    pub command: String,
    pub error: EngineError,
}

/// Run the steps in order, stopping at the first error; returns each
/// completed step's result.
pub fn run(session: &mut Session, steps: &[Step]) -> std::result::Result<Vec<Value>, (Vec<Value>, StepError)> {
    let mut results = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        match session.execute(&step.command, step.args.clone()) {
            Ok(out) => results.push(out),
            Err(error) => return Err((results, StepError { index, command: step.command.clone(), error })),
        }
    }
    Ok(results)
}
