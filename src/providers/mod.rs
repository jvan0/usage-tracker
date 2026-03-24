// providers/mod.rs — Module index + factories

mod antigravity;
mod chatgpt;
mod claude;
mod cursor;
mod kilocode;
mod opencode;

pub use antigravity::AntigravityProvider;
pub use chatgpt::ChatGPTProvider;
pub use claude::ClaudeProvider;
pub use cursor::CursorProvider;
pub use kilocode::KiloCodeProvider;
pub use opencode::OpenCodeProvider;

use crate::provider::Provider;

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(ClaudeProvider),
        Box::new(ChatGPTProvider),
        Box::new(AntigravityProvider),
        Box::new(KiloCodeProvider),
        Box::new(CursorProvider),
        Box::new(OpenCodeProvider),
    ]
}

pub fn get_provider(name: &str) -> Option<Box<dyn Provider>> {
    match name {
        "claude" => Some(Box::new(ClaudeProvider)),
        "chatgpt" => Some(Box::new(ChatGPTProvider)),
        "antigravity" => Some(Box::new(AntigravityProvider)),
        "kilocode" => Some(Box::new(KiloCodeProvider)),
        "cursor" => Some(Box::new(CursorProvider)),
        "opencode" => Some(Box::new(OpenCodeProvider)),
        _ => None,
    }
}
