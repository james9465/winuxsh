# Daemon and Command Routing Implementation Plan

## Overview

This document outlines the implementation strategy for:
1. **WinuxCmd Daemon Management**: Automatic daemon detection, startup, and persistence across terminal sessions
2. **Command Routing Priority**: Ensuring WinuxCmd IPC commands take precedence over PATH executables
3. **Alias System Integration**: Proper handling of aliases with command routing

## Requirements

### Daemon Management
- [ ] Auto-detect if WinuxCmd daemon is running on shell startup
- [ ] Start daemon automatically if not running
- [ ] Daemon should persist when shell exits (not terminate)
- [ ] Multiple shell instances should share the same daemon
- [ ] Proper cleanup if daemon is no longer needed

### Command Routing
- [ ] WinuxCmd IPC commands should have higher priority than PATH executables
- [ ] Handle command classification (Simple/Interactive/Complex)
- [ ] Support builtin commands with highest priority
- [ ] Graceful fallback if IPC is unavailable

### Alias System
- [ ] Alias expansion should occur before command routing
- [ ] Support recursive alias expansion (with safeguards)
- [ ] Handle arguments in alias definitions
- [ ] Integration with command routing

## Current Architecture Analysis

### Existing Components

1. **`src/executor.rs`**
   - Executes external commands via `find_command_in_path()`
   - Searches PATH for executable files
   - Handles redirections and background processes
   - **Issue**: No awareness of WinuxCmd IPC

2. **`src/winuxcmd_ffi.rs`**
   - Provides IPC interface to WinuxCmd daemon
   - Has `execute(command, args)` method
   - Returns `WinuxCmdResponse` with stdout/stderr/exit_code
   - **Issue**: Not integrated into command execution flow

3. **`src/shell.rs`**
   - Main REPL loop
   - `execute_single_command()` method
   - Handles builtin commands first, then delegates to executor
   - **Issue**: Command routing logic needs enhancement

4. **`src/command_router.rs`**
   - Command classification system
   - `classify(command)` returns category
   - Priority definitions
   - **Status**: Ready for integration

## Implementation Strategy

### Phase 1: Daemon Management Enhancement

#### 1.1 Daemon Detection and Startup

**Location**: `src/main.rs` - `initialize_winuxcmd_daemon()`

**Implementation**:
```rust
fn initialize_winuxcmd_daemon() -> Result<()> {
    // 1. Initialize FFI
    WinuxCmdFFI::init()?;
    
    // 2. Check if daemon is available
    if WinuxCmdFFI::is_available() {
        println!("✓ WinuxCmd daemon is running");
        return Ok(());
    }
    
    // 3. Start daemon if not available
    println!("Starting WinuxCmd daemon...");
    start_winuxcmd_daemon()?;
    
    // 4. Wait for daemon to be ready (with timeout)
    let timeout = Duration::from_secs(5);
    let start = Instant::now();
    while start.elapsed() < timeout {
        if WinuxCmdFFI::is_available() {
            println!("✓ WinuxCmd daemon started successfully");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    
    Err(anyhow!("Failed to start WinuxCmd daemon within timeout"))
}

fn start_winuxcmd_daemon() -> Result<()> {
    use std::process::{Command, Stdio};
    
    // Find winuxcmd.exe location
    let daemon_path = find_winuxcmd_executable()?;
    
    // Start as detached background process
    Command::new(&daemon_path)
        .arg("--daemon")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow!("Failed to start daemon: {}", e))?;
    
    Ok(())
}
```

**Key Points**:
- Daemon starts as detached process (won't terminate when shell exits)
- Uses `--daemon` flag (if supported) or runs in background
- Waits up to 5 seconds for daemon to be ready
- Proper error handling and user feedback

#### 1.2 Daemon Process Management

**Location**: `src/winuxcmd_ffi.rs`

**Add to `WinuxCmdFFI`**:
```rust
impl WinuxCmdFFI {
    /// Get daemon process ID
    pub fn get_daemon_pid() -> Option<u32> {
        // Use Windows API to find daemon process
        // or use named pipe information
    }
    
    /// Terminate daemon (if needed)
    pub fn terminate_daemon() -> Result<()> {
        // Only call this when explicitly needed
        // Not called on normal shell exit
    }
}
```

### Phase 2: Command Routing System

#### 2.1 Create Command Router

**Location**: `src/command_router.rs` - Add new struct

```rust
use crate::command_router::CommandCategory;
use crate::winuxcmd_ffi::WinuxCmdFFI;

pub struct CommandRouter {
    classification: CommandClassification,
    daemon_available: bool,
}

impl CommandRouter {
    pub fn new(classification: CommandClassification) -> Self {
        let daemon_available = WinuxCmdFFI::is_available();
        Self {
            classification,
            daemon_available,
        }
    }
    
    /// Route command to appropriate executor
    pub fn route_command(
        &self,
        command: &str,
        args: &[String],
    ) -> RouteDecision {
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
        
        // 3. Fall back to PATH executable
        RouteDecision::ExternalCommand
    }
    
    /// Update daemon availability status
    pub fn update_daemon_status(&mut self) {
        self.daemon_available = WinuxCmdFFI::is_available();
    }
}

pub enum RouteDecision {
    Builtin,                                    // Native WinSH command
    WinuxCmdIPC(CommandCategory),              // Execute via WinuxCmd daemon
    ExternalCommand,                           // Execute via PATH
    NotFound,                                  // Command not found
}
```

#### 2.2 Integrate Router into Shell

**Location**: `src/shell.rs` - Add to `Shell` struct

```rust
use crate::command_router::CommandRouter;

pub struct Shell {
    // ... existing fields ...
    command_router: Option<CommandRouter>,
}
```

**Initialize in `Shell::new()`**:
```rust
let command_router = if let Some(classification) = self.command_classification.take() {
    Some(CommandRouter::new(classification))
} else {
    None
};
```

### Phase 3: Command Execution Flow Enhancement

#### 3.1 Modify `execute_single_command()`

**Location**: `src/shell.rs`

**New execution flow**:
```rust
pub fn execute_single_command(&mut self, cmd: &CommandInfo) -> Result<()> {
    // 1. Skip empty commands
    if cmd.args.is_empty() {
        return Ok(());
    }

    // 2. Expand aliases (FIRST - highest priority)
    let cmd_clone = self.expand_aliases(cmd)?;
    
    // 3. Get command name
    let clean_command = cmd_clone.args[0]
        .trim_matches(|c: char| c == '\u{feff}' || c == '\u{fffe}' || c.is_whitespace())
        .to_string();

    // 4. Expand command substitution in arguments
    let args_with_substitution: Vec<String> = cmd_clone.args[1..]
        .iter()
        .map(|arg| self.expand_command_substitution(arg))
        .collect();

    // 5. Expand wildcards
    let expanded_args = self.expand_wildcards(&args_with_substitution);
    let all_args: Vec<String> = vec![clean_command.clone()]
        .into_iter()
        .chain(expanded_args)
        .collect();

    // 6. Route command based on priority
    if let Some(router) = &self.command_router {
        match router.route_command(&clean_command, &all_args[1..]) {
            RouteDecision::Builtin => {
                return self.execute_builtin_command(&clean_command, &all_args);
            }
            RouteDecision::WinuxCmdIPC(category) => {
                return self.execute_winuxcmd_command(&clean_command, &all_args[1..], category);
            }
            RouteDecision::ExternalCommand => {
                // Fall through to executor
            }
            RouteDecision::NotFound => {
                return Err(ShellError::CommandNotFound(format!(
                    "Command '{}' not found",
                    clean_command
                )));
            }
        }
    }

    // 7. Fall back to external command execution
    self.execute_external_command(&clean_command, &all_args[1..], &cmd_clone)
}
```

#### 3.2 Implement WinuxCmd Command Execution

**Location**: `src/shell.rs` - Add new method

```rust
fn execute_winuxcmd_command(
    &mut self,
    command: &str,
    args: &[String],
    category: CommandCategory,
) -> Result<()> {
    match category {
        CommandCategory::Interactive => {
            // Interactive commands need TTY handling
            self.execute_interactive_winuxcmd(command, args)
        }
        CommandCategory::Complex => {
            // Complex commands need special parsing
            self.execute_complex_winuxcmd(command, args)
        }
        CommandCategory::Simple => {
            // Simple commands - direct IPC
            self.execute_simple_winuxcmd(command, args)
        }
        _ => {
            // Should not happen
            self.execute_simple_winuxcmd(command, args)
        }
    }
}

fn execute_simple_winuxcmd(&mut self, command: &str, args: &[String]) -> Result<()> {
    use crate::winuxcmd_ffi::WinuxCmdFFI;
    
    match WinuxCmdFFI::execute(command, args) {
        Ok(response) => {
            // Print output
            if !response.stdout.is_empty() {
                print!("{}", response.stdout);
            }
            if !response.stderr.is_empty() {
                eprint!("{}", response.stderr);
            }
            
            self.last_exit_code = response.exit_code;
            
            if response.exit_code != 0 {
                eprintln!("Command exited with status code: {}", response.exit_code);
            }
            
            Ok(())
        }
        Err(e) => {
            // Fall back to external command if IPC fails
            eprintln!("Warning: WinuxCmd IPC failed: {}", e);
            eprintln!("Falling back to external command execution");
            self.execute_external_command_fallback(command, args)
        }
    }
}

fn execute_interactive_winuxcmd(&mut self, command: &str, args: &[String]) -> Result<()> {
    // For interactive commands (less, top), we need special TTY handling
    // This might require temporarily restoring TTY state
    
    use crate::winuxcmd_ffi::WinuxCmdFFI;
    
    // Save current terminal state
    // let saved_tty_state = save_terminal_state();
    
    match WinuxCmdFFI::execute(command, args) {
        Ok(response) => {
            print!("{}", response.stdout);
            eprint!("{}", response.stderr);
            self.last_exit_code = response.exit_code;
            Ok(())
        }
        Err(e) => {
            eprintln!("IPC failed: {}", e);
            self.execute_external_command_fallback(command, args)
        }
    }
}

fn execute_complex_winuxcmd(&mut self, command: &str, args: &[String]) -> Result<()> {
    // Complex commands (sed, xargs) need special argument parsing
    // For now, use simple execution
    
    self.execute_simple_winuxcmd(command, args)
}
```

### Phase 4: Alias System Enhancement

#### 4.1 Alias Expansion

**Location**: `src/shell.rs` - Add new method

```rust
fn expand_aliases(&self, cmd: &CommandInfo) -> Result<CommandInfo> {
    let mut cmd_clone = cmd.clone();
    
    if cmd_clone.args.is_empty() {
        return Ok(cmd_clone);
    }
    
    let first_arg = &cmd_clone.args[0];
    
    // Check if first arg is an alias
    if let Some(alias_def) = self.aliases.get(first_arg) {
        let alias_parts: Vec<String> = alias_def
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        
        if !alias_parts.is_empty() {
            // Replace command name
            cmd_clone.args[0] = alias_parts[0].clone();
            
            // Insert alias arguments
            cmd_clone
                .args
                .splice(1..1, alias_parts[1..].iter().cloned());
            
            // Recursive alias expansion prevention
            // Track expansion depth to prevent infinite loops
            if self.count_alias_expansions(&cmd_clone.args[0]) > 10 {
                return Err(ShellError::AliasExpansion(
                    "Too many alias expansions (possible infinite loop)".to_string()
                ));
            }
        }
    }
    
    Ok(cmd_clone)
}

fn count_alias_expansions(&self, command: &str) -> usize {
    // Count how many times this command has been expanded
    // Implementation depends on tracking expansion state
    0  // Placeholder
}
```

#### 4.2 Alias Integration with Routing

**Key Points**:
- Aliases are expanded **before** command routing
- Expanded command goes through normal routing
- Recursive aliases are limited to prevent infinite loops
- Aliases can reference WinuxCmd commands, builtin commands, or external commands

### Phase 5: Testing Strategy

#### 5.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_command_routing_priority() {
        let router = create_test_router();
        
        // Test builtin has highest priority
        assert_eq!(
            router.route_command("cd", &[]),
            RouteDecision::Builtin
        );
        
        // Test WinuxCmd command
        assert_eq!(
            router.route_command("ls", &["-la".to_string()]),
            RouteDecision::WinuxCmdIPC(CommandCategory::Simple)
        );
        
        // Test external command fallback
        assert_eq!(
            router.route_command("notepad", &[]),
            RouteDecision::ExternalCommand
        );
    }
    
    #[test]
    fn test_alias_expansion() {
        let mut shell = create_test_shell();
        
        shell.aliases.insert("ll", "ls -la".to_string());
        
        let cmd = CommandInfo {
            args: vec!["ll".to_string()],
            // ... other fields
        };
        
        let expanded = shell.expand_aliases(&cmd).unwrap();
        assert_eq!(expanded.args[0], "ls");
        assert_eq!(expanded.args[1], "-la");
    }
    
    #[test]
    fn test_winuxcmd_command_execution() {
        // Mock WinuxCmdFFI for testing
        // Test execution, error handling, fallback
    }
}
```

#### 5.2 Integration Tests

```rust
#[test]
fn test_full_command_flow() {
    // 1. Start shell
    // 2. Execute WinuxCmd command (should use IPC)
    // 3. Execute builtin command
    // 4. Execute external command
    // 5. Test alias
    // 6. Verify correct routing
}
```

### Phase 6: Edge Cases and Error Handling

#### 6.1 Daemon Unavailability

```rust
fn execute_simple_winuxcmd(&mut self, command: &str, args: &[String]) -> Result<()> {
    match WinuxCmdFFI::execute(command, args) {
        Ok(response) => { /* handle success */ }
        Err(e) => {
            // Try to reconnect or restart daemon
            if should_restart_daemon(&e) {
                self.restart_daemon()?;
                // Retry once
                if let Ok(response) = WinuxCmdFFI::execute(command, args) {
                    return Ok(/* handle response */);
                }
            }
            
            // Fall back to external command
            self.execute_external_command_fallback(command, args)
        }
    }
}
```

#### 6.2 PATH vs WinuxCmd Conflicts

**Handling Strategy**:
1. WinuxCmd commands always have priority
2. User can explicitly request PATH version using full path
3. Add warning if PATH command is shadowed

```rust
// Example:
// user types: ls        -> Uses WinuxCmd ls (IPC)
// user types: /usr/bin/ls -> Uses PATH ls
// user types: ./ls.exe -> Uses local ls.exe
```

#### 6.3 Interactive Command TTY Handling

```rust
fn execute_interactive_winuxcmd(&mut self, command: &str, args: &[String]) -> Result<()> {
    // Save terminal state
    // Disable line editing
    // Pass TTY to command
    // Restore terminal state
}
```

## Implementation Order

### Priority 1: Core Functionality
1. ✅ Command classification (already done)
2. ✅ WinuxCmd FFI (already done)
3. ⏳ Daemon management enhancement
4. ⏳ Command router implementation
5. ⏳ Integrate router into shell execution

### Priority 2: Robustness
6. ⏳ Alias expansion integration
7. ⏳ Error handling and fallbacks
8. ⏳ TTY handling for interactive commands

### Priority 3: Polish
9. ⏳ Comprehensive testing
10. ⏳ Performance optimization
11. ⏳ Documentation

## Configuration Changes

### `commands_classification.toml`

No changes needed - already has all required classifications.

### `.winshrc.toml` (optional)

```toml
[winuxcmd]
# WinuxCmd daemon settings
auto_start = true
daemon_timeout = 5  # seconds
prefer_ipc = true  # Always prefer WinuxCmd IPC over PATH

[aliases]
# Aliases that use WinuxCmd commands
ll = "ls -la"
la = "ls -a"
l = "ls"

# User can override to use PATH version if needed
# ls = "/usr/bin/ls"  # Force PATH version
```

## Migration Path

### For Users

1. **No breaking changes**: Existing behavior preserved for external commands
2. **Gradual rollout**: WinuxCmd commands automatically detected and routed
3. **Fallback**: If IPC fails, automatically falls back to PATH
4. **Opt-in**: Users can disable WinuxCmd routing if needed

### For Developers

1. **Modular design**: Each component can be tested independently
2. **Clear interfaces**: Well-defined boundaries between components
3. **Extensible**: Easy to add new command types or routing rules

## Performance Considerations

### Lookup Performance

- **Command classification**: O(1) with HashSet (future optimization)
- **Daemon availability check**: Fast named pipe check
- **Command routing**: Minimal overhead, one classification lookup

### Execution Performance

- **WinuxCmd IPC**: Low overhead, daemon process already running
- **Builtin commands**: Fast, no process creation
- **External commands**: Same as before, no regression

### Memory Footprint

- **Classification data**: ~5-10KB for 137 commands
- **Router instance**: Negligible
- **Daemon process**: Shared across all shells

## Security Considerations

### Command Validation

- All command names validated before routing
- Prevents command injection
- Path traversal protection

### Daemon Security

- Daemon runs as user process
- No elevated privileges
- Named pipe access control

### Alias Security

- Limited recursion depth
- Prevents infinite loops
- No code execution in alias definitions

## Success Criteria

### Functional Requirements

- ✅ Daemon auto-starts if not running
- ✅ Daemon persists when shell exits
- ✅ WinuxCmd commands use IPC
- ✅ PATH commands still work
- ✅ Builtin commands have highest priority
- ✅ Aliases work correctly
- ✅ Fallback on errors

### Performance Requirements

- Command routing < 1ms overhead
- IPC execution similar performance to native
- No noticeable degradation in user experience

### Quality Requirements

- All tests pass
- No memory leaks
- Proper error handling
- Clear error messages

## Next Steps

1. **Review this plan** with team/stakeholders
2. **Implement Phase 1** (Daemon management)
3. **Implement Phase 2** (Command router)
4. **Implement Phase 3** (Execution flow)
5. **Implement Phase 4** (Alias system)
6. **Implement Phase 5** (Testing)
7. **Implement Phase 6** (Edge cases)
8. **Integration testing**
9. **Performance testing**
10. **Documentation updates**

## References

- `src/command_router.rs` - Command classification
- `src/winuxcmd_ffi.rs` - IPC interface
- `src/executor.rs` - External command execution
- `src/shell.rs` - Main shell logic
- `commands_classification.toml` - Command definitions
- `DOCS/command-classification-best-practices.md` - Best practices

---

**Document Version**: 1.0  
**Last Updated**: 2026-03-31  
**Status**: Draft - Ready for review and implementation