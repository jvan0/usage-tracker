// =============================================================================
// config.rs — Configuración persistente (config.toml)
// =============================================================================
//
// Conceptos nuevos:
//   - toml crate → parsear archivos TOML
//   - dirs crate → encontrar directorios del sistema
//   - Config file pattern → leer config, aplicar defaults
//   - tokio::time::interval → auto-refresh periódico
// =============================================================================

use serde::Deserialize;

/// Configuración del tracker
///
/// Se lee de ~/.config/usage-tracker/config.toml (Linux/Mac)
/// o %APPDATA%\usage-tracker\config.toml (Windows)
///
/// #[serde(default)] en cada campo permite que el archivo TOML
/// solo especifique lo que quiere cambiar. Los demás usan defaults.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    /// Providers habilitados. Si está vacío, usa todos.
    #[serde(default)]
    pub enabled_providers: Vec<String>,

    /// Intervalo de refresh en segundos (para el comando watch)
    #[serde(default = "default_refresh_secs")]
    pub refresh_secs: u64,

    /// Mostrar notificaciones cuando usage supera umbral
    #[serde(default)]
    pub notify_threshold: Option<i32>,
}

fn default_refresh_secs() -> u64 {
    300 // 5 minutos
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            enabled_providers: vec![
                "claude".to_string(),
                "chatgpt".to_string(),
                "antigravity".to_string(),
                "kilocode".to_string(),
                "cursor".to_string(),
                "opencode".to_string(),
            ],
            refresh_secs: 300,
            notify_threshold: None,
        }
    }
}

/// Carga la config desde el archivo, o retorna default si no existe.
///
/// ¿Qué hace unwrap_or_default()?
///   Si load_config() retorna Err, usa AppConfig::default().
///   No rompe, no panic. Usa defaults sensatos.
pub fn load_config() -> AppConfig {
    match find_config_path() {
        Some(path) if path.exists() => match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("Warning: error parseando config {:?}: {}", path, e);
                AppConfig::default()
            }),
            Err(e) => {
                eprintln!("Warning: no se pudo leer config {:?}: {}", path, e);
                AppConfig::default()
            }
        },
        _ => AppConfig::default(),
    }
}

/// Crea un config.toml de ejemplo si no existe.
pub fn create_default_config() -> Result<std::path::PathBuf, String> {
    let path = find_config_path().ok_or("No se pudo determinar directorio de config")?;

    if path.exists() {
        return Ok(path);
    }

    // Crear directorio padre si no existe
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Error creando directorio de config: {}", e))?;
    }

    let default_config = r#"# usage-tracker config
# Editá este archivo para personalizar el comportamiento.

# Providers habilitados (comentar para deshabilitar)
enabled_providers = ["claude", "chatgpt", "antigravity", "opencode"]

# Intervalo de refresh en segundos (default: 300 = 5 min)
refresh_secs = 300

# Notificar cuando usage supere este porcentaje (opcional)
# notify_threshold = 80
"#;

    std::fs::write(&path, default_config)
        .map_err(|e| format!("Error escribiendo config: {}", e))?;

    Ok(path)
}

fn find_config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("usage-tracker").join("config.toml"))
}
