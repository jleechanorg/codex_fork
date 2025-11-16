//! Codex Extensions System
//!
//! This crate provides extensibility features for the Codex CLI:
//! - Slash commands: User-defined commands via markdown files
//! - Hooks: Lifecycle event hooks via executable scripts
//! - Settings: Configuration management for extensions

pub mod error;
pub mod hooks;
pub mod settings;
pub mod slash_commands;

pub use error::{ExtensionError, Result};
pub use hooks::HookSystem;
pub use settings::Settings;
pub use slash_commands::SlashCommandRegistry;
