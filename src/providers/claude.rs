// =============================================================================
// providers/claude.rs — Claude Provider REAL (HTTP + OAuth)
// =============================================================================
//
// ¡Esto es lo que separa un script de un programa REAL!
//
// ClaudeProvider ahora:
//   1. Lee el OAuth token de ~/.claude/.credentials.json
//   2. Hace un GET a la API de Anthropic
//   3. Parsea el JSON de respuesta
//   4. Retorna ProviderUsage con datos REALES
//
// Conceptos nuevos:
//   - async fn + .await
//   - reqwest (HTTP client)
//   - serde Deserialize (JSON → struct)
//   - ? operator (propagación de errores)
//   - std::fs (filesystem)
// =============================================================================

use async_trait::async_trait;
use serde::Deserialize;

use crate::provider::{Provider, ProviderUsage};

// --- STRUCTS DE LA API DE CLAUDE --------------------------------------------
//
// #[derive(Deserialize)] le dice a serde: "generá el código para convertir
// JSON en instancias de este struct automáticamente".
//
// ¿Cómo sabe qué campo mapear a qué?
//   Por NOMBRE. El campo "access_token" en el JSON se mapea al campo
//   access_token del struct. Si el nombre del JSON es diferente,
//   usás #[serde(rename = "nombre_en_json")]
//
// #[allow(dead_code)] = "no me avises si no uso todos los campos".
// La API devuelve más campos de los que necesitamos. No queremos warnings.

/// Estructura del archivo ~/.claude/.credentials.json
///
/// IMPORTANTE: el formato real es:
/// { "claudeAiOauth": { "accessToken": "sk-ant-oat01-...", ... } }
/// NO es { "access_token": "..." }. ¡Siempre verificá el JSON real!
#[derive(Deserialize, Debug)]
struct ClaudeCredentials {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: ClaudeOAuth,
}

#[derive(Deserialize, Debug)]
struct ClaudeOAuth {
    #[serde(rename = "accessToken")]
    access_token: String,
}

/// Respuesta de GET /api/oauth/usage
#[derive(Deserialize, Debug)]
struct ClaudeUsageResponse {
    five_hour: Option<ClaudeWindow>,
    seven_day: Option<ClaudeWindow>,
}

/// Una ventana de rate limit (5h o 7d)
///
/// IMPORTANTE: la API devuelve "utilization" (ya en porcentaje, ej: 49.0 = 49%),
/// NO "fraction_used" (que sería 0.49). ¡Siempre verificá el JSON real!
#[derive(Deserialize, Debug)]
struct ClaudeWindow {
    /// Utilización en porcentaje (0-100). Ej: 49.0 = 49%
    utilization: Option<f64>,
    /// ISO-8601 timestamp del reset
    resets_at: Option<String>,
}

// --- CLAUDE PROVIDER --------------------------------------------------------
pub struct ClaudeProvider;

#[async_trait]
impl Provider for ClaudeProvider {
    fn name(&self) -> &str {
        "Claude"
    }

    // async fn fetch() → necesitás .await cuando llamás esta función.
    // Si no ponés .await, obtenés un Future (promesa), no el resultado.
    async fn fetch(&self) -> Result<ProviderUsage, String> {
        // --- PASO 1: Leer el token ---
        let token = read_claude_token()?;

        // --- PASO 2: Hacer el HTTP call ---
        let response = call_claude_api(&token).await?;

        // --- PASO 3: Mapear a ProviderUsage ---
        Ok(claude_response_to_usage(response))
    }
}

// --- PASO 1: LEER TOKEN -----------------------------------------------------
//
// ¿Qué es el ? operator?
//   Es un ATAJO para match en errores.
//
//   Sin ?:
//     let content = match std::fs::read_to_string(&path) {
//         Ok(c) => c,
//         Err(e) => return Err(e.to_string()),
//     };
//
//   Con ?:
//     let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
//
//   ? = "si es Err, retorná el error AHORA. Si es Ok, dame el valor".
//   Es el equivalente a try/catch, pero como VALOR, no como control flow.
//
// ¿Qué es map_err?
//   Convierte un error de un tipo a otro.
//   std::fs::read_to_string retorna Result<String, std::io::Error>.
//   Nosotros queremos Result<String, String>.
//   map_err convierte el io::Error en String.
fn read_claude_token() -> Result<String, String> {
    // dirs::home_dir() no está disponible sin crate "dirs".
    // Usamos std::env::var("HOME") o %USERPROFILE% en Windows.
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "No se pudo determinar el directorio HOME".to_string())?;

    // ~/.claude/.credentials.json (nota: el archivo tiene punto al principio)
    let cred_path = std::path::Path::new(&home)
        .join(".claude")
        .join(".credentials.json");

    // Leer el archivo
    let content = std::fs::read_to_string(&cred_path).map_err(|e| {
        format!(
            "No se encontró {:?}. Instalá Claude CLI primero (error: {})",
            cred_path, e
        )
    })?;

    // Parsear JSON → ClaudeCredentials
    // serde_json::from_str convierte un String JSON en un struct tipado.
    // Si el JSON no tiene el campo "access_token", retorna Err.
    let creds: ClaudeCredentials = serde_json::from_str(&content)
        .map_err(|e| format!("Error parseando credentials.json: {}", e))?;

    Ok(creds.claude_ai_oauth.access_token)
}

// --- PASO 2: HTTP CALL ------------------------------------------------------
//
// async fn = función asíncrona. Cuando la llamás, obtenés un Future.
// Para obtener el resultado real, necesitás .await.
//
// ¿Qué hace .await?
//   "Esperá a que esta operación asíncrona termine, pero SIN bloquear el thread".
//   Mientras espera, el runtime (tokio) puede ejecutar otras tareas.
//   Es como await en JavaScript.
async fn call_claude_api(token: &str) -> Result<ClaudeUsageResponse, String> {
    // reqwest::Client es el HTTP client.
    // Es como fetch() en JS, pero con más features.
    let client = reqwest::Client::new();

    // .get(url) → crea un GET request builder
    // .header(k, v) → agrega headers
    // .send().await → envía el request (async!)
    // .json::<T>().await → deserializa el body JSON en tipo T (async!)
    //
    // NOTA: anthropic-beta header es requerido por la API de OAuth usage.
    // Sin él, la API retorna 400.
    let response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {}", token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|e| format!("Error conectando a Claude API: {}", e))?;

    // Verificar que el status sea 200 OK
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Claude API error {}: {}", status, body));
    }

    // .json::<T>() deserializa el body JSON directamente al struct.
    // Es magia de serde + reqwest trabajando juntos.
    response
        .json::<ClaudeUsageResponse>()
        .await
        .map_err(|e| format!("Error parseando respuesta de Claude: {}", e))
}

// --- PASO 3: MAPEAR RESPUESTA → PROVIDER USAGE -----------------------------
fn claude_response_to_usage(response: ClaudeUsageResponse) -> ProviderUsage {
    // five_hour → session_pct
    // NOTA: utilization ya viene en porcentaje (49.0 = 49%), NO es fracción.
    let (session_pct, reset_time) = match response.five_hour {
        Some(window) => {
            // utilization es 49.0 → queremos 49 (i32)
            // as i32 trunca: 49.7 → 49. Es suficiente para un porcentaje.
            let pct = window.utilization.map(|u| u as i32);
            let reset = window.resets_at
                .map(|r| format_resets_at(&r))
                .unwrap_or_else(|| "unknown".to_string());
            (pct, reset)
        }
        None => (None, "N/A".to_string()),
    };

    // seven_day → weekly_pct
    let weekly_pct = response.seven_day
        .and_then(|w| w.utilization.map(|u| u as i32));

    ProviderUsage {
        name: "Claude".to_string(),
        session_pct,
        weekly_pct,
        reset_time,
    }
}

// Helper: convierte ISO-8601 timestamp a formato "Xh Ym restantes"
fn format_resets_at(iso: &str) -> String {
    // Parsear el timestamp ISO-8601
    // Formato: "2026-03-24T03:00:00.031304+00:00"
    //
    // Estrategia simple: extraer fecha+hora, calcular diferencia con "ahora".
    // No usamos chrono para no agregar dependencia.
    //
    // Formato del output: "1h 30m" o "reset soon" si es menos de 1 minuto

    // Parsear manualmente: "2026-03-24T03:00:00"
    let clean = iso.split('+').next().unwrap_or(iso); // quitar timezone offset
    let clean = clean.split('.').next().unwrap_or(clean); // quitar microsegundos

    if let Some((_date_part, time_part)) = clean.split_once('T') {
        // Parsear hora
        let parts: Vec<&str> = time_part.split(':').collect();
        if parts.len() >= 2 {
            let reset_hour: u32 = parts[0].parse().unwrap_or(0);
            let reset_min: u32 = parts[1].parse().unwrap_or(0);

            // Hora actual UTC (aproximada)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let total_secs = now.as_secs();
            let current_hour = (total_secs % 86400) / 3600;
            let current_min = (total_secs % 3600) / 60;

            // Diferencia en minutos
            let reset_total_min = reset_hour * 60 + reset_min;
            let current_total_min = (current_hour as u32) * 60 + (current_min as u32);

            let diff_min = if reset_total_min >= current_total_min {
                reset_total_min - current_total_min
            } else {
                // Reset es mañana
                (24 * 60 - current_total_min) + reset_total_min
            };

            if diff_min < 1 {
                return "reset soon".to_string();
            }

            let hours = diff_min / 60;
            let mins = diff_min % 60;

            if hours > 0 {
                return format!("{}h {:02}m", hours, mins);
            } else {
                return format!("{}m", mins);
            }
        }
    }

    // Fallback: mostrar la hora del string
    if let Some(time_part) = iso.split('T').nth(1) {
        let hm: String = time_part.chars().take(5).collect();
        return format!("{}", hm);
    }

    iso.to_string()
}
