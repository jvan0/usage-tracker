// providers/cursor.rs — Cursor Provider
//
// Cursor es un fork de VS Code con IA integrada.
// Detectamos si está instalado buscando ~/.cursor/

use async_trait::async_trait;
use crate::provider::{Provider, ProviderUsage};

pub struct CursorProvider;

#[async_trait]
impl Provider for CursorProvider {
    fn name(&self) -> &str {
        "Cursor"
    }

    async fn fetch(&self) -> Result<ProviderUsage, String> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| "No se pudo determinar HOME".to_string())?;

        let cursor_dir = std::path::Path::new(&home).join(".cursor");
        let installed = cursor_dir.exists();

        Ok(ProviderUsage {
            name: "Cursor".to_string(),
            session_pct: None,
            weekly_pct: None,
            reset_time: if installed {
                "no API".to_string()
            } else {
                "not installed".to_string()
            },
        })
    }
}
