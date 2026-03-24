// providers/kilocode.rs — Kilo Code Provider
//
// Kilo Code es una extensión de VS Code. No tiene API de usage pública.
// Detectamos si está instalado buscando la extensión o el config.

use async_trait::async_trait;
use crate::provider::{Provider, ProviderUsage};

pub struct KiloCodeProvider;

#[async_trait]
impl Provider for KiloCodeProvider {
    fn name(&self) -> &str {
        "Kilo Code"
    }

    async fn fetch(&self) -> Result<ProviderUsage, String> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| "No se pudo determinar HOME".to_string())?;

        let kilo_dir = std::path::Path::new(&home).join(".config").join("kilo");
        let alt_dir = std::path::Path::new(&home).join(".kilocode");

        let installed = kilo_dir.exists() || alt_dir.exists();

        Ok(ProviderUsage {
            name: "Kilo Code".to_string(),
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
