// =============================================================================
// providers/antigravity.rs — Antigravity Local Probe
// =============================================================================
//
// Antigravity (Windsurf/Codeium) corre un language server LOCAL.
// Para obtener usage, necesitamos:
//   1. Detectar el proceso del language server
//   2. Extraer --csrf_token y --extension_server_port de sus argumentos
//   3. Descubrir en qué puerto está escuchando
//   4. POST al endpoint local con el token
//   5. Parsear la respuesta
//
// Conceptos nuevos:
//   - std::process::Command → ejecutar comandos del sistema
//   - regex crate → expresiones regulares
//   - cfg!(target_os) → código condicional por plataforma
//   - reqwest con accept_invalid_certs → para HTTPS self-signed en localhost
// =============================================================================

use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;

use crate::provider::{Provider, ProviderUsage};

// --- STRUCTS DE LA RESPUESTA DE ANTIGRAVITY ---------------------------------
//
// La estructura exacta de la respuesta JSON del language server.
// Usamos #[serde(default)] y Option<T> para ser defensivos —
// si un campo falta, no rompe.
#[derive(Deserialize, Debug, Default)]
struct AntigravityResponse {
    #[serde(default)]
    user_status: Option<UserStatus>,
    #[serde(default)]
    cascade_model_config_data: Option<CascadeConfig>,
}

#[derive(Deserialize, Debug)]
struct UserStatus {
    #[serde(default)]
    cascade_model_config_data: Option<CascadeConfig>,
    #[allow(dead_code)]
    account_email: Option<String>,
    #[allow(dead_code)]
    plan_name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CascadeConfig {
    #[serde(default, rename = "clientModelConfigs")]
    client_model_configs: Option<Vec<ModelConfig>>,
}

#[derive(Deserialize, Debug)]
struct ModelConfig {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    quota_info: Option<QuotaInfo>,
}

#[derive(Deserialize, Debug)]
struct QuotaInfo {
    #[serde(default, rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(default, rename = "resetTime")]
    reset_time: Option<String>,
}

// --- PROCESS INFO -----------------------------------------------------------
struct ProcessInfo {
    csrf_token: String,
    port: u16,
}

// --- ANTIGRAVITY PROVIDER ---------------------------------------------------
pub struct AntigravityProvider;

#[async_trait]
impl Provider for AntigravityProvider {
    fn name(&self) -> &str {
        "Antigravity"
    }

    async fn fetch(&self) -> Result<ProviderUsage, String> {
        // 1. Detectar proceso
        let info = detect_antigravity_process()?;

        // 2. Llamar al endpoint local
        let data = call_local_api(&info).await?;

        // 3. Parsear response
        Ok(parse_response(data))
    }
}

// --- DETECTAR PROCESO DEL LANGUAGE SERVER -----------------------------------
//
// std::process::Command ejecuta un comando del sistema y captura su output.
// Es como subprocess.run() en Python o exec() en otros lenguajes.
//
// cfg!(target_os = "...") es COMPILATION TIME, no runtime.
// El compilador elige qué rama compilar según el target OS.
// No hay overhead — la otra rama ni siquiera existe en el binario.
fn detect_antigravity_process() -> Result<ProcessInfo, String> {
    let output = if cfg!(target_os = "windows") {
        // Windows: tasklist no muestra argumentos. Usamos wmic o Get-Process.
        // Para simplicidad, usamos PowerShell.
        std::process::Command::new("powershell")
            .args(["-Command", "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -match 'language_server' -and $_.CommandLine -match 'antigravity' } | Select-Object -ExpandProperty CommandLine"])
            .output()
            .map_err(|e| format!("Error ejecutando PowerShell: {}", e))?
    } else {
        // Linux/Mac: ps muestra la línea de comandos completa.
        // -ax = todos los procesos, -o pid=,command= = solo PID y comando.
        std::process::Command::new("ps")
            .args(["-ax", "-o", "pid=,command="])
            .output()
            .map_err(|e| format!("Error ejecutando ps: {}", e))?
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Buscar líneas que contengan "language_server" y "antigravity"
    let cmdline = stdout
        .lines()
        .find(|line| {
            line.contains("language_server")
                && (line.contains("antigravity") || line.contains("--app_data_dir"))
        })
        .ok_or_else(|| {
            "Antigravity no está corriendo. Abrí Antigravity/Windsurf primero.".to_string()
        })?;

    // Extraer csrf_token y port con regex
    let csrf_token = extract_arg(cmdline, r"--csrf_token\s+(\S+)")?;
    let port_str = extract_arg(cmdline, r"--extension_server_port\s+(\d+)")?;
    let port: u16 = port_str
        .parse()
        .map_err(|e| format!("Puerto inválido '{}': {}", port_str, e))?;

    Ok(ProcessInfo { csrf_token, port })
}

// --- EXTRAER ARGUMENTO CON REGEX --------------------------------------------
//
// regex::Regex::new() crea una expresión regular.
// .captures() busca el primer match y captura los grupos.
// .get(1) obtiene el primer grupo capturado (el que está entre paréntesis).
//
// ¿Por qué regex y no simplemente .split("--csrf_token ")?
//   Porque los argumentos pueden estar en cualquier orden,
//   pueden tener espacios, etc. Regex es más robusto.
fn extract_arg(cmdline: &str, pattern: &str) -> Result<String, String> {
    let re = Regex::new(pattern)
        .map_err(|e| format!("Error en regex '{}': {}", pattern, e))?;

    re.captures(cmdline)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| format!("No se encontró '{}' en: {}", pattern, cmdline))
}

// --- LLAMAR AL API LOCAL ----------------------------------------------------
//
// El language server de Antigravity expone endpoints HTTP/2 gRPC.
// Pero también responde HTTP/1.1 con JSON para compatibilidad.
//
// IMPORTANTE: usa HTTPS con certificado self-signed en localhost.
// reqwest::Client::builder().danger_accept_invalid_certs(true)
// le dice al cliente que acepte cualquier certificado.
// ¡NUNCA hagas esto para APIs externas! Solo para localhost.
async fn call_local_api(info: &ProcessInfo) -> Result<AntigravityResponse, String> {
    let url = format!(
        "https://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetUnleashData",
        info.port
    );

    // Cliente que acepta self-signed certs SOLO para localhost
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Error creando HTTP client: {}", e))?;

    // El body es un JSON mínimo con metadata
    let body = serde_json::json!({
        "metadata": {
            "ideName": "antigravity",
            "extensionName": "antigravity",
            "locale": "en",
            "ideVersion": "unknown"
        }
    });

    let response = client
        .post(&url)
        .header("X-Codeium-Csrf-Token", &info.csrf_token)
        .header("Connect-Protocol-Version", "1")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            format!(
                "No se pudo conectar a Antigravity en {}. ¿Está corriendo? Error: {}",
                url, e
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Antigravity API error {}: {}", status, body_text));
    }

    // Intentamos parsear la respuesta. Si falla, retornamos un default.
    // Esto es defensive parsing — no rompemos si el formato cambió.
    let text = response
        .text()
        .await
        .map_err(|e| format!("Error leyendo respuesta: {}", e))?;

    serde_json::from_str(&text).map_err(|e| {
        format!(
            "Error parseando respuesta de Antigravity: {}. Body: {}",
            e,
            &text[..text.len().min(200)]
        )
    })
}

// --- PARSEAR RESPONSE → PROVIDER USAGE -------------------------------------
//
// La respuesta tiene model configs con quota info.
// Buscamos el modelo principal (Claude o Gemini) y extraemos su usage.
fn parse_response(data: AntigravityResponse) -> ProviderUsage {
    // Intentar extraer configs del user_status o directamente
    let configs = data
        .user_status
        .as_ref()
        .and_then(|us| us.cascade_model_config_data.as_ref())
        .or(data.cascade_model_config_data.as_ref())
        .and_then(|c| c.client_model_configs.as_ref());

    match configs {
        Some(models) => {
            // Buscar el modelo principal (Claude si existe)
            let main_model = models
                .iter()
                .find(|m| {
                    m.label
                        .as_ref()
                        .map(|l| l.to_lowercase().contains("claude"))
                        .unwrap_or(false)
                })
                .or_else(|| models.first()); // fallback al primer modelo

            match main_model {
                Some(model) => {
                    let session_pct = model
                        .quota_info
                        .as_ref()
                        .and_then(|q| q.remaining_fraction)
                        .map(|f| ((1.0 - f) * 100.0) as i32); // remaining → used

                    let reset_time = model
                        .quota_info
                        .as_ref()
                        .and_then(|q| q.reset_time.as_ref())
                        .map(|r| format_reset_time(r))
                        .unwrap_or("N/A".to_string());

                    ProviderUsage {
                        name: "Antigravity".to_string(),
                        session_pct,
                        weekly_pct: None, // Antigravity no tiene ventana semanal
                        reset_time,
                    }
                }
                None => ProviderUsage {
                    name: "Antigravity".to_string(),
                    session_pct: None,
                    weekly_pct: None,
                    reset_time: "no data".to_string(),
                },
            }
        }
        None => ProviderUsage {
            name: "Antigravity".to_string(),
            session_pct: None,
            weekly_pct: None,
            reset_time: "no models found".to_string(),
        },
    }
}

fn format_reset_time(iso: &str) -> String {
    if let Some(time_part) = iso.split('T').nth(1) {
        // Extraer HH:MM del timestamp ISO
        let parts: Vec<&str> = time_part.split(':').take(2).collect();
        if parts.len() == 2 {
            return format!("{}:{}", parts[0], parts[1]);
        }
    }
    iso.to_string()
}
