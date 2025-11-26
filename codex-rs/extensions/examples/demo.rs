//! Demo program showing the extension system in action
//!
//! Run with: cargo run --example demo

use codex_extensions::HookSystem;
use codex_extensions::Settings;
use codex_extensions::SlashCommandRegistry;
use codex_extensions::hooks::HookEvent;
use codex_extensions::hooks::HookInput;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    println!("=== Codex Extensions Demo ===\n");

    // Use examples/claude as the project directory
    // Try multiple paths to find examples/claude
    let possible_paths = vec![
        std::path::PathBuf::from("../../examples/claude"),
        std::path::PathBuf::from("../examples/claude"),
        std::path::PathBuf::from("examples/claude"),
    ];

    let project_dir = possible_paths
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| {
            eprintln!("Error: examples/claude directory not found");
            eprintln!("Please run from the repository root");
            std::process::exit(1);
        });

    // Load settings directly from examples/claude (not .claude subdirectory)
    println!("1. Loading settings from settings.json...");
    let settings_path = project_dir.join("settings.json");
    match Settings::load_from_file(&settings_path) {
        Ok(settings) => {
            println!("   ✓ Settings loaded successfully");
            println!(
                "   - Hook events configured: {:?}",
                settings.hooks.keys().collect::<Vec<_>>()
            );
            if let Some(status_line) = &settings.status_line {
                println!("   - Status line command: {}", status_line.command);
            }
        }
        Err(e) => {
            eprintln!("   ✗ Failed to load settings: {}", e);
        }
    }

    println!();

    // Load slash commands directly from commands directory
    println!("2. Loading slash commands from commands/...");
    let mut registry = SlashCommandRegistry::new();
    let commands_dir = project_dir.join("commands");
    match registry.load_from_directory(&commands_dir) {
        Ok(()) => {
            println!("   ✓ Loaded {} commands:", registry.list().len());
            for cmd in registry.list() {
                println!(
                    "     - /{}: {}",
                    cmd.metadata.name, cmd.metadata.description
                );
            }

            println!();
            println!("3. Testing slash command detection:");
            let test_inputs = vec![
                "/hello World",
                "/echo test message",
                "regular text without slash",
                "/help",
            ];

            for input in test_inputs {
                if let Some((cmd_name, args)) = SlashCommandRegistry::detect_command(input) {
                    println!("   ✓ Detected: /{} with args: '{}'", cmd_name, args);
                    if let Some(cmd) = registry.get(&cmd_name) {
                        let substituted = cmd.substitute_arguments(&args);
                        println!(
                            "     Content preview: {}",
                            substituted.lines().take(2).collect::<Vec<_>>().join(" ")
                        );
                    }
                } else {
                    println!("   - No command in: {}", input);
                }
            }
        }
        Err(e) => {
            eprintln!("   ✗ Failed to load commands: {}", e);
        }
    }

    println!();

    // Initialize hook system
    // Note: HookSystem expects a project directory that contains .claude/
    // Since our examples/claude IS the .claude directory, we pass its parent
    println!("\n4. Initializing hook system...");
    let hook_root = project_dir
        .parent()
        .unwrap_or(project_dir.as_path())
        .to_path_buf();
    match HookSystem::new(Some(&hook_root)) {
        Ok(hook_system) => {
            println!("   ✓ Hook system initialized");

            println!();
            println!("5. Testing hook execution (UserPromptSubmit):");

            let input = HookInput {
                session_id: "demo-session".to_string(),
                transcript_path: "".to_string(),
                cwd: project_dir.to_string_lossy().to_string(),
                hook_event_name: "UserPromptSubmit".to_string(),
                extra: HashMap::new(),
            };

            match hook_system
                .execute(HookEvent::UserPromptSubmit, input)
                .await
            {
                Ok(results) => {
                    if results.is_empty() {
                        println!("   - No hooks configured for UserPromptSubmit");
                    } else {
                        println!("   ✓ Executed {} hook(s):", results.len());
                        for (i, result) in results.iter().enumerate() {
                            println!("     Hook {}: exit code {}", i + 1, result.exit_code);
                            if let Some(output) = &result.parsed_output {
                                println!("       Output: {:?}", output);
                            }
                            if result.is_blocking() {
                                println!("       ⚠ This hook would block the action!");
                                if let Some(reason) = result.block_reason() {
                                    println!("       Reason: {}", reason);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("   ✗ Hook execution failed: {}", e);
                }
            }

            println!();
            println!("6. Testing SessionStart hook:");
            let session_start_input = HookInput {
                session_id: "demo-session".to_string(),
                transcript_path: "".to_string(),
                cwd: project_dir.to_string_lossy().to_string(),
                hook_event_name: "SessionStart".to_string(),
                extra: HashMap::new(),
            };

            match hook_system
                .execute(HookEvent::SessionStart, session_start_input)
                .await
            {
                Ok(results) => {
                    if results.is_empty() {
                        println!("   - No hooks configured for SessionStart");
                    } else {
                        println!("   ✓ Executed {} hook(s)", results.len());
                        for result in results {
                            if !result.stdout.is_empty() {
                                println!("     stdout: {}", result.stdout.trim());
                            }
                            if !result.stderr.is_empty() {
                                println!("     stderr: {}", result.stderr.trim());
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("   ✗ Hook execution failed: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("   ✗ Failed to initialize hook system: {}", e);
        }
    }

    println!();
    println!("=== Demo Complete ===");
    println!("\nTo use these extensions in your own code:");
    println!("1. Copy examples/claude/ to ~/.claude/");
    println!("2. Use SlashCommandRegistry::load() to load commands");
    println!("3. Use HookSystem::new() to initialize hooks");
    println!("4. Call hook_system.execute() at appropriate lifecycle points");
}
