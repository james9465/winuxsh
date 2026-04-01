// Custom completer for WinSH
// Integrates command, path, and variable completion

use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use reedline::{Completer, Span, Suggestion};
use crate::completion::{CompletionContext, CompletionResult};
use crate::completion::command::CommandCompleter;
use crate::completion::path::PathCompleter;
use crate::completion::variables::VariableCompleter;
use crate::array::ArrayValue;

/// State shared with completer
#[derive(Clone)]
pub struct CompletionState {
    pub current_dir: PathBuf,
    pub env_vars: HashMap<String, ArrayValue>,
}

impl CompletionState {
    pub fn new(current_dir: PathBuf) -> Self {
        Self {
            current_dir,
            env_vars: HashMap::new(),
        }
    }
}

/// Custom completer for WinSH
pub struct WinuxshCompleter {
    state: Arc<Mutex<CompletionState>>,
}

impl WinuxshCompleter {
    /// Create a new completer with shared state
    pub fn new(state: Arc<Mutex<CompletionState>>) -> Self {
        Self {
            state,
        }
    }

    /// Update state
    pub fn update_state(&self, current_dir: PathBuf, env_vars: HashMap<String, ArrayValue>) {
        if let Ok(mut state) = self.state.lock() {
            state.current_dir = current_dir;
            state.env_vars = env_vars;
        }
    }

    /// Complete input
    fn complete_input(&mut self, input: &str, cursor_pos: usize) -> Vec<Suggestion> {
        let (current_dir, env_vars) = if let Ok(state) = self.state.lock() {
            (state.current_dir.clone(), state.env_vars.clone())
        } else {
            return Vec::new();
        };

        let context = CompletionContext::new(current_dir.clone(), input.to_string(), cursor_pos);

        // Try different completion strategies
        if context.is_path_completion() {
            if let Ok(Some(result)) = PathCompleter::complete(&context) {
                return self.format_completions(result, input, cursor_pos);
            }
        } else if context.is_variable_completion() {
            if let Ok(Some(result)) = VariableCompleter::complete(&context, &env_vars) {
                return self.format_completions(result, input, cursor_pos);
            }
        } else {
            // Try command completion
            if let Ok(Some(result)) = CommandCompleter::complete(&context) {
                return self.format_completions(result, input, cursor_pos);
            }
        }

        Vec::new()
    }

    /// Format completions as suggestions
    fn format_completions(&self, result: CompletionResult, input: &str, cursor_pos: usize) -> Vec<Suggestion> {
        let completions = result.completions;

        // Calculate span for the word being completed
        // Find the start of the current word
        let word_start = input[..cursor_pos]
            .rfind(|c: char| c.is_whitespace() || c == ';' || c == '|' || c == '&')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        // For path completion, we need to handle path separators correctly
        // Only replace the part after the last path separator
        let before_cursor = &input[..cursor_pos];
        let last_path_sep = before_cursor.rfind(|c: char| c == '/' || c == '\\');
        
        let span = if let Some(sep_pos) = last_path_sep {
            // For paths, only replace after the last separator
            Span {
                start: sep_pos + 1,
                end: cursor_pos,
            }
        } else {
            // For commands and variables, replace the whole word
            Span {
                start: word_start,
                end: cursor_pos,
            }
        };

        completions
            .into_iter()
            .map(|c| Suggestion {
                value: c,
                span: span.clone(),
                ..Default::default()
            })
            .collect()
    }
}

impl Completer for WinuxshCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        self.complete_input(line, pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completer_creation() {
        let state = Arc::new(Mutex::new(CompletionState::new(PathBuf::from("/home/user"))));
        let completer = WinuxshCompleter::new(state);
    }
}
