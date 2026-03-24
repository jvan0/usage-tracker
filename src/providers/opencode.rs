// providers/opencode.rs — OpenCode Provider
//
// OpenCode es un tool de desarrollo local, similar a Antigravity.
// No tiene cuotas de usage como ChatGPT o Claude.
// Mostramos N/A pero verificamos si está instalado.
//
// Concepto: #[serde(default)] en cada nivel para defensive parsing.
// Si el JSON no tiene lo que buscamos, no rompe.

use async_trait::async_trait;
use crate::provider::{Provider, ProviderUsage};

pub struct OpenCodeProvider;

#[async_trait]
impl Provider for OpenCodeProvider {
    fn name(&self) -> &str {
        "OpenCode"
    }

    async fn fetch(&self) -> Result<ProviderUsage, String> {
        // Verificar si OpenCode está instalado buscando su config
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| "No se pudo determinar HOME".to_string())?;

        // Buscar en ~/.config/opencode/ o ~/.opencode/
        let config_path = std::path::Path::new(&home)
            .join(".config")
            .join("opencode")
            .join("opencode.json");

        let alt_path = std::path::Path::new(&home)
            .join(".opencode")
            .join("opencode.json");

        let installed = config_path.exists() || alt_path.exists();

        // OpenCode no tiene API de usage. Mostramos N/A pero con info de instalación.
        Ok(ProviderUsage {
            name: "OpenCode".to_string(),
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
