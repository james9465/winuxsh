// Custom completer for WinSH
// Integrates command, path, and variable completion

use std::path::PathBuf;
use std::collections::HashMap;
use reedline::{Completer, Span, Suggestion};
use crate::completion::{CompletionContext, CompletionResult};
use crate::completion::command::CommandCompleter;
use crate::completion::path::PathCompleter;
use crate::completion::variables::VariableCompleter;
use crate::array::ArrayValue;
use crate::error::Result;

/// Custom completer for WinSH
pub struct WinuxshCompleter {
    current_dir: PathBuf,
    env_vars: HashMap<String, ArrayValue>,
}

impl WinuxshCompleter {
    /// Create a new completer
    pub fn new(current_dir: PathBuf, env_vars: HashMap<String, ArrayValue>) -> Self {
        Self {
            current_dir,
            env_vars,
        }
    }

    /// Update current directory
    pub fn update_current_dir(&mut self, dir: PathBuf) {
        self.current_dir = dir;
    }

    /// Update environment variables
    pub fn update_env_vars(&mut self, vars: HashMap<String, ArrayValue>) {
        self.env_vars = vars;
    }

    /// Complete input
    fn complete_input(&mut self, input: &str, cursor_pos: usize) -> Vec<Suggestion> {
        let context = CompletionContext::new(self.current_dir.clone(), input.to_string(), cursor_pos);

        // Try different completion strategies
        if context.is_path_completion() {
            if let Ok(Some(result)) = PathCompleter::complete(&context) {
                return self.format_completions(result, input, cursor_pos);
            }
        } else if context.is_variable_completion() {
            if let Ok(Some(result)) = VariableCompleter::complete(&context, &self.env_vars) {
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
        let word_start = input[..cursor_pos]
            .rfind(|c: char| c.is_whitespace() || c == ';' || c == '|' || c == '&')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        let span = Span {
            start: word_start,
            end: cursor_pos,
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
        let completer = WinuxshCompleter::new(
            PathBuf::from("/home/user"),
            HashMap::new(),
        );
        assert_eq!(completer.current_dir, PathBuf::from("/home/user"));
    }
}