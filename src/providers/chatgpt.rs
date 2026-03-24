// =============================================================================
// providers/chatgpt.rs — ChatGPT/Codex Provider REAL (HTTP + OAuth)
// =============================================================================
//
// API: GET https://chatgpt.com/backend-api/wham/usage
// Token: ~/.codex/auth.json → tokens.access_token
//
// Response format (real):
// {
//   "rate_limit": {
//     "primary_window": { "used_percent": 0, "reset_after_seconds": 18000 },
//     "secondary_window": { "used_percent": 69, "reset_after_seconds": 230373 }
//   }
// }
//
// primary_window  = sesión de 5 horas (5h = 18000 segundos)
// secondary_window = ventana semanal (7d = 604800 segundos)
// =============================================================================

use async_trait::async_trait;
use serde::Deserialize;

use crate::provider::{Provider, ProviderUsage};

// --- STRUCTS DE AUTH.JSON ---------------------------------------------------
//
// El formato real de ~/.codex/auth.json:
// {
//   "auth_mode": "chatgpt",
//   "tokens": {
//     "access_token": "eyJ...",
//     "refresh_token": "rt_...",
//     "account_id": "..."
//   }
// }
//
// NOTA: usamos #[serde(default)] en campos opcionales para que no falle
// si algún campo falta. Defensive parsing — nunca asumas que el JSON tiene todo.
#[derive(Deserialize, Debug)]
struct CodexAuth {
    tokens: Option<CodexTokens>,
}

#[derive(Deserialize, Debug)]
struct CodexTokens {
    access_token: Option<String>,
}

// --- STRUCTS DE LA API DE CHATGPT ------------------------------------------
//
// Mirá qué lindo: la estructura del JSON se mapea DIRECTO a estos structs.
// serde hace todo el trabajo de deserialización.
//
// primary_window.reset_after_seconds → cuántos segundos hasta el reset
// primary_window.used_percent → porcentaje usado (0-100)
#[derive(Deserialize, Debug)]
struct ChatGPTUsageResponse {
    #[serde(default)]
    rate_limit: Option<RateLimit>,
}

#[derive(Deserialize, Debug)]
struct RateLimit {
    primary_window: Option<Window>,
    secondary_window: Option<Window>,
}

#[derive(Deserialize, Debug)]
struct Window {
    used_percent: Option<i32>,
    reset_after_seconds: Option<i64>,
}

// --- CHATGPT PROVIDER -------------------------------------------------------
pub struct ChatGPTProvider;

#[async_trait]
impl Provider for ChatGPTProvider {
    fn name(&self) -> &str {
        "ChatGPT"
    }

    async fn fetch(&self) -> Result<ProviderUsage, String> {
        let token = read_chatgpt_token()?;
        let response = call_chatgpt_api(&token).await?;
        Ok(chatgpt_response_to_usage(response))
    }
}

// --- LEER TOKEN DE AUTH.JSON ------------------------------------------------
//
// Mismo pattern que Claude, pero con un path y formato diferente.
// ¿Ves cómo el trait nos da ESTRUCTURA? Cada provider tiene las mismas 3 funciones
// internas (read token, call API, map response), pero implementadas diferente.
fn read_chatgpt_token() -> Result<String, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "No se pudo determinar el directorio HOME".to_string())?;

    let auth_path = std::path::Path::new(&home)
        .join(".codex")
        .join("auth.json");

    let content = std::fs::read_to_string(&auth_path).map_err(|e| {
        format!(
            "No se encontró {:?}. Instalá Codex CLI primero (error: {})",
            auth_path, e
        )
    })?;

    let auth: CodexAuth = serde_json::from_str(&content)
        .map_err(|e| format!("Error parseando auth.json: {}", e))?;

    // .ok_or() convierte Option → Result
    // None → Err("mensaje")
    // Some(valor) → Ok(valor)
    //
    // ¿Por qué no usamos ? acá?
    // Porque ? es para Result. Para Option → Result, usamos .ok_or() o .ok_or_else().
    let token = auth
        .tokens
        .ok_or("auth.json no tiene campo 'tokens'")?
        .access_token
        .ok_or("auth.json no tiene tokens.access_token")?;

    Ok(token)
}

// --- HTTP CALL A CHATGPT ----------------------------------------------------
//
// Patrón idéntico a Claude: client + get + header + send + json.
// La diferencia: URL y headers diferentes.
async fn call_chatgpt_api(token: &str) -> Result<ChatGPTUsageResponse, String> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Error conectando a ChatGPT API: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("ChatGPT API error {}: {}", status, body));
    }

    response
        .json::<ChatGPTUsageResponse>()
        .await
        .map_err(|e| format!("Error parseando respuesta de ChatGPT: {}", e))
}

// --- MAPEAR RESPONSE --------------------------------------------------------
//
// reset_after_seconds → formato humano "Xh Ym"
fn chatgpt_response_to_usage(response: ChatGPTUsageResponse) -> ProviderUsage {
    let (session_pct, reset_time) = match response.rate_limit {
        Some(ref rl) => {
            let session = rl.primary_window
                .as_ref()
                .and_then(|w| w.used_percent);

            let reset = rl.primary_window
                .as_ref()
                .and_then(|w| w.reset_after_seconds)
                .map(format_seconds)
                .unwrap_or("N/A".to_string());

            (session, reset)
        }
        None => (None, "N/A".to_string()),
    };

    let weekly_pct = response.rate_limit
        .as_ref()
        .and_then(|rl| rl.secondary_window.as_ref())
        .and_then(|w| w.used_percent);

    ProviderUsage {
        name: "ChatGPT".to_string(),
        session_pct,
        weekly_pct,
        reset_time,
    }
}

// Helper: convierte segundos a formato "Xh Ym"
fn format_seconds(seconds: i64) -> String {
    if seconds <= 0 {
        return "now".to_string();
    }
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{}h {:02}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}
