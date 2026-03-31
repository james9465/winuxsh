use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum CommandCategory {
    Simple,
    Interactive,
    Complex,
    Builtin,
}

#[derive(Debug, Deserialize)]
pub struct CommandClassification {
    #[serde(rename = "simple_commands")]
    pub simple: SimpleCommands,
    #[serde(rename = "interactive_commands")]
    pub interactive: InteractiveCommands,
    #[serde(rename = "complex_commands")]
    pub complex: ComplexCommands,
    #[serde(rename = "builtin_commands")]
    pub builtin: BuiltinCommands,
    #[serde(rename = "command_priority")]
    pub priority: CommandPriority,
}

#[derive(Debug, Deserialize)]
pub struct SimpleCommands {
    pub simple: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InteractiveCommands {
    pub interactive: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ComplexCommands {
    pub complex: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct BuiltinCommands {
    pub builtin: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommandPriority {
    pub builtin: u32,
    pub simple: u32,
    pub complex: u32,
    pub interactive: u32,
}

impl CommandClassification {
    pub fn classify(&self, command: &str) -> Option<CommandCategory> {
        // Check builtin first (highest priority)
        if self.simple.simple.contains(&command.to_string()) {
            return Some(CommandCategory::Simple);
        }
        if self.interactive.interactive.contains(&command.to_string()) {
            return Some(CommandCategory::Interactive);
        }
        if self.complex.complex.contains(&command.to_string()) {
            return Some(CommandCategory::Complex);
        }
        if self.builtin.builtin.contains(&command.to_string()) {
            return Some(CommandCategory::Builtin);
        }
        None
    }

    pub fn get_priority(&self, category: &CommandCategory) -> u32 {
        match category {
            CommandCategory::Builtin => self.priority.builtin,
            CommandCategory::Simple => self.priority.simple,
            CommandCategory::Complex => self.priority.complex,
            CommandCategory::Interactive => self.priority.interactive,
        }
    }

    pub fn is_winuxcmd_command(&self, command: &str) -> bool {
        self.simple.simple.contains(&command.to_string())
            || self.interactive.interactive.contains(&command.to_string())
            || self.complex.complex.contains(&command.to_string())
    }

    pub fn is_builtin_command(&self, command: &str) -> bool {
        self.builtin.builtin.contains(&command.to_string())
    }

    pub fn is_interactive(&self, command: &str) -> bool {
        self.interactive.interactive.contains(&command.to_string())
    }
}

pub fn load_classification() -> Result<CommandClassification> {
    let config_path = "commands_classification.toml";
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| anyhow!("Failed to read classification file: {}", e))?;
    
    let classification: CommandClassification = toml::from_str(&content)
        .map_err(|e| anyhow!("Failed to parse classification file: {}", e))?;
    
    Ok(classification)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_classification() {
        let classification = load_classification().unwrap();
        
        // Test known commands
        assert_eq!(classification.classify("ls"), Some(CommandCategory::Simple));
        assert_eq!(classification.classify("grep"), Some(CommandCategory::Simple));
        assert_eq!(classification.classify("less"), Some(CommandCategory::Interactive));
        assert_eq!(classification.classify("top"), Some(CommandCategory::Interactive));
        assert_eq!(classification.classify("sed"), Some(CommandCategory::Complex));
        assert_eq!(classification.classify("xargs"), Some(CommandCategory::Complex));
        assert_eq!(classification.classify("cd"), Some(CommandCategory::Builtin));
        
        // Test unknown command
        assert_eq!(classification.classify("nonexistent"), None);
    }

    #[test]
    fn test_is_winuxcmd_command() {
        let classification = load_classification().unwrap();
        
        assert!(classification.is_winuxcmd_command("ls"));
        assert!(classification.is_winuxcmd_command("grep"));
        assert!(classification.is_winuxcmd_command("less"));
        assert!(classification.is_winuxcmd_command("sed"));
        
        assert!(!classification.is_winuxcmd_command("cd"));
        assert!(!classification.is_winuxcmd_command("exit"));
    }

    #[test]
    fn test_is_builtin_command() {
        let classification = load_classification().unwrap();
        
        assert!(classification.is_builtin_command("cd"));
        assert!(classification.is_builtin_command("exit"));
        assert!(classification.is_builtin_command("pwd"));
        
        assert!(!classification.is_builtin_command("ls"));
        assert!(!classification.is_builtin_command("grep"));
    }

    #[test]
    fn test_is_interactive() {
        let classification = load_classification().unwrap();
        
        assert!(classification.is_interactive("less"));
        assert!(classification.is_interactive("top"));
        
        assert!(!classification.is_interactive("ls"));
        assert!(!classification.is_interactive("grep"));
    }

    #[test]
    fn test_get_priority() {
        let classification = load_classification().unwrap();
        
        assert_eq!(classification.get_priority(&CommandCategory::Builtin), 1);
        assert_eq!(classification.get_priority(&CommandCategory::Simple), 2);
        assert_eq!(classification.get_priority(&CommandCategory::Complex), 3);
        assert_eq!(classification.get_priority(&CommandCategory::Interactive), 4);
    }
}

/// Route decision for command execution
#[derive(Debug, Clone, PartialEq)]
pub enum RouteDecision {
    /// Native WinSH builtin command (highest priority)
    Builtin,
    /// Execute via WinuxCmd daemon IPC
    WinuxCmdIPC(CommandCategory),
    /// Execute via PATH as external command
    ExternalCommand,
    /// Command not found
    NotFound,
}

/// Command router for determining how to execute commands
pub struct CommandRouter {
    classification: CommandClassification,
    daemon_available: bool,
}

impl CommandRouter {
    /// Create a new command router
    pub fn new(classification: CommandClassification) -> Self {
        let daemon_available = crate::winuxcmd_ffi::WinuxCmdFFI::is_available();
        Self {
            classification,
            daemon_available,
        }
    }

    /// Route a command to the appropriate executor
    ///
    /// Routing priority:
    /// 1. Builtin commands (highest)
    /// 2. WinuxCmd IPC commands (if daemon available)
    /// 3. External commands from PATH (lowest)
    pub fn route_command(&self, command: &str) -> RouteDecision {
        // Check if command contains path separator - use external execution
        if command.contains('\\') || command.contains('/') {
            return RouteDecision::ExternalCommand;
        }

        // 1. Check builtin first (highest priority)
        if self.classification.is_builtin_command(command) {
            return RouteDecision::Builtin;
        }

        // 2. Check WinuxCmd IPC (if daemon available)
        if self.daemon_available {
            if let Some(category) = self.classification.classify(command) {
                return RouteDecision::WinuxCmdIPC(category);
            }
        }

        // 3. Fall back to external command execution
        // We don't return NotFound here because the command might exist in PATH
        RouteDecision::ExternalCommand
    }

    /// Update daemon availability status
    pub fn update_daemon_status(&mut self) {
        self.daemon_available = crate::winuxcmd_ffi::WinuxCmdFFI::is_available();
    }

    /// Check if daemon is available
    pub fn is_daemon_available(&self) -> bool {
        self.daemon_available
    }

    /// Get reference to command classification
    pub fn classification(&self) -> &CommandClassification {
        &self.classification
    }
}

#[cfg(test)]
mod router_tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let classification = load_classification().unwrap();
        let router = CommandRouter::new(classification);
        // Just test creation works
        assert_eq!(router.classification().priority.builtin, 1);
    }

    #[test]
    fn test_route_builtin() {
        let classification = load_classification().unwrap();
        let router = CommandRouter::new(classification);
        
        assert_eq!(
            router.route_command("cd"),
            RouteDecision::Builtin
        );
        assert_eq!(
            router.route_command("pwd"),
            RouteDecision::Builtin
        );
        assert_eq!(
            router.route_command("echo"),
            RouteDecision::Builtin
        );
    }

    #[test]
    fn test_route_winuxcmd_simple() {
        let classification = load_classification().unwrap();
        let router = CommandRouter::new(classification);
        
        if router.is_daemon_available() {
            assert_eq!(
                router.route_command("ls"),
                RouteDecision::WinuxCmdIPC(CommandCategory::Simple)
            );
            assert_eq!(
                router.route_command("grep"),
                RouteDecision::WinuxCmdIPC(CommandCategory::Simple)
            );
        } else {
            // Fallback to external if daemon not available
            assert_eq!(
                router.route_command("ls"),
                RouteDecision::ExternalCommand
            );
        }
    }

    #[test]
    fn test_route_winuxcmd_interactive() {
        let classification = load_classification().unwrap();
        let router = CommandRouter::new(classification);
        
        if router.is_daemon_available() {
            assert_eq!(
                router.route_command("less"),
                RouteDecision::WinuxCmdIPC(CommandCategory::Interactive)
            );
            assert_eq!(
                router.route_command("top"),
                RouteDecision::WinuxCmdIPC(CommandCategory::Interactive)
            );
        }
    }

    #[test]
    fn test_route_winuxcmd_complex() {
        let classification = load_classification().unwrap();
        let router = CommandRouter::new(classification);
        
        if router.is_daemon_available() {
            assert_eq!(
                router.route_command("sed"),
                RouteDecision::WinuxCmdIPC(CommandCategory::Complex)
            );
            assert_eq!(
                router.route_command("xargs"),
                RouteDecision::WinuxCmdIPC(CommandCategory::Complex)
            );
        }
    }

    #[test]
    fn test_route_external() {
        let classification = load_classification().unwrap();
        let router = CommandRouter::new(classification);
        
        // Commands not in classification should route to external
        assert_eq!(
            router.route_command("notepad"),
            RouteDecision::ExternalCommand
        );
        assert_eq!(
            router.route_command("unknowncmd"),
            RouteDecision::ExternalCommand
        );
    }

    #[test]
    fn test_route_with_path() {
        let classification = load_classification().unwrap();
        let router = CommandRouter::new(classification);
        
        // Commands with path separators should use external execution
        assert_eq!(
            router.route_command("C:\\Git\\bin\\ls.exe"),
            RouteDecision::ExternalCommand
        );
        assert_eq!(
            router.route_command("./ls.exe"),
            RouteDecision::ExternalCommand
        );
    }
}