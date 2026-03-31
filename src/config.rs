// Configuration management for WinSH
use crate::error::{Result, ShellError};
use std::path::{Path, PathBuf};

/// Terminal color configuration
#[derive(Debug, Clone)]
pub struct TerminalColors {
    pub prompt_user: String,
    pub prompt_host: String,
    pub prompt_dir: String,
    pub prompt_symbol: String,
}

impl Default for TerminalColors {
    fn default() -> Self {
        TerminalColors {
            prompt_user: "\x1b[1;32m".to_string(), // Green
            prompt_host: "\x1b[1;32m".to_string(), // Green
            prompt_dir: "\x1b[1;34m".to_string(),  // Blue
            prompt_symbol: "\x1b[0m".to_string(),  // Reset
        }
    }
}

/// Shell configuration
#[derive(Debug, Clone)]
pub struct ShellConfig {
    pub colors: TerminalColors,
    pub prompt_format: String,
    pub plugins: Vec<String>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig {
            colors: TerminalColors::default(),
            prompt_format: "%u@%h %w $ ".to_string(),
            plugins: Vec::new(),
        }
    }
}

/// Configuration manager
pub struct ConfigManager {
    config: ShellConfig,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        ConfigManager {
            config: ShellConfig::default(),
        }
    }

    /// Load configuration from file
    pub fn load_config(&mut self, path: &Path) -> Result<ShellConfig> {
        let config_content = std::fs::read_to_string(path)
            .map_err(|e| ShellError::Config(format!("Failed to read config: {}", e)))?;

        // Parse key=value format
        for line in config_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = self.parse_ansi_escape_sequences(value.trim());

                match key {
                    "prompt_user_color" => {
                        self.config.colors.prompt_user = value;
                    }
                    "prompt_host_color" => {
                        self.config.colors.prompt_host = value;
                    }
                    "prompt_dir_color" => {
                        self.config.colors.prompt_dir = value;
                    }
                    "prompt_symbol" => {
                        self.config.colors.prompt_symbol = value;
                    }
                    "prompt_format" => {
                        self.config.prompt_format = value;
                    }
                    "plugin" => {
                        self.config.plugins.push(value);
                    }
                    _ => {
                        // Ignore other keys for now
                    }
                }
            }
        }

        Ok(self.config.clone())
    }

    /// Get configuration
    pub fn config(&self) -> &ShellConfig {
        &self.config
    }

    /// Parse ANSI escape sequences like \x1b[1;32m
    fn parse_ansi_escape_sequences(&self, input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().peekable();

        while let Some(&c) = chars.peek() {
            chars.next();

            if c == '\\' {
                if let Some(&'x') = chars.peek() {
                    chars.next();
                    // Parse hex escape sequence
                    let mut hex_str = String::new();
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_hexdigit() && hex_str.len() < 2 {
                            chars.next();
                            hex_str.push(c);
                        } else {
                            break;
                        }
                    }

                    if let Ok(byte) = u8::from_str_radix(&hex_str, 16) {
                        result.push(byte as char);
                    }
                } else {
                    // Keep backslash for other escape sequences
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Find configuration file in standard locations
    pub fn find_config_file() -> Option<PathBuf> {
        let home_dir = dirs::home_dir()?;
        let config_paths = vec![
            PathBuf::from(".winshrc.toml"),
            home_dir.join(".winshrc.toml"),
            PathBuf::from(".winshrc"),
            home_dir.join(".winshrc"),
        ];

        config_paths.into_iter().find(|path| path.exists())
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ShellConfig::default();
        assert_eq!(config.prompt_format, "%u@%h %w $ ");
        assert_eq!(config.plugins.len(), 0);
    }

    #[test]
    fn test_default_colors() {
        let colors = TerminalColors::default();
        assert_eq!(colors.prompt_user, "\x1b[1;32m");
        assert_eq!(colors.prompt_host, "\x1b[1;32m");
        assert_eq!(colors.prompt_dir, "\x1b[1;34m");
        assert_eq!(colors.prompt_symbol, "\x1b[0m");
    }

    #[test]
    fn test_ansi_parsing() {
        let manager = ConfigManager::new();
        let result = manager.parse_ansi_escape_sequences("\\x1b[1;32m");
        assert_eq!(result, "\x1b[1;32m");
    }
}
