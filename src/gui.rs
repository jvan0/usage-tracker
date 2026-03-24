// =============================================================================
// gui.rs — App de escritorio con egui (v3: Connections con Auth)
// =============================================================================

use crate::provider::ProviderUsage;
use crate::providers::all_providers;
use eframe::egui;
use serde::Deserialize;

// --- APP STATE --------------------------------------------------------------
pub struct UsageTrackerApp {
    providers: Vec<ProviderUsage>,
    errors: Vec<(String, String)>,
    last_update: Option<std::time::Instant>,
    loading: bool,
    refresh_secs: u64,
    active_tab: Tab,
    connections: Vec<ConnectionStatus>,
    disconnected: std::collections::HashSet<String>,
    rt: Option<tokio::runtime::Runtime>,
    /// Modo compacto (widget siempre visible)
    compact: bool,
}

#[derive(PartialEq, Clone)]
enum Tab {
    Overview,
    Connections,
    Settings,
}

#[derive(Clone)]
struct ConnectionStatus {
    name: String,
    status: AuthState,
    detail: String,
    email: Option<String>,
}

#[derive(Clone, PartialEq)]
enum AuthState {
    Connected,
    Expired,
    NotConfigured,
}

// --- APP CREATION -----------------------------------------------------------
impl UsageTrackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        let rt = tokio::runtime::Runtime::new().ok();

        let mut app = UsageTrackerApp {
            providers: Vec::new(),
            errors: Vec::new(),
            last_update: None,
            loading: false,
            refresh_secs: 60,
            active_tab: Tab::Overview,
            connections: Vec::new(),
            disconnected: std::collections::HashSet::new(),
            rt,
            compact: false,
        };

        app.check_connections();
        app
    }

    /// Crear versión compacta (widget siempre visible)
    pub fn new_widget(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        let rt = tokio::runtime::Runtime::new().ok();

        let mut app = UsageTrackerApp {
            providers: Vec::new(),
            errors: Vec::new(),
            last_update: None,
            loading: false,
            refresh_secs: 30, // refresh más frecuente en widget
            active_tab: Tab::Overview,
            connections: Vec::new(),
            disconnected: std::collections::HashSet::new(),
            rt,
            compact: true, // modo compacto
        };

        app.check_connections();
        app
    }

    fn check_connections(&mut self) {
        let mut conns = vec![
            check_claude_connection(),
            check_chatgpt_connection(),
            check_antigravity_connection(),
            check_kilocode_connection(),
            check_cursor_connection(),
            check_opencode_connection(),
        ];

        // Marcar como desconectados los que el usuario desconectó
        for conn in &mut conns {
            if self.disconnected.contains(&conn.name) {
                conn.status = AuthState::NotConfigured;
                conn.detail = "Disconnected by user".to_string();
                conn.email = None;
            }
        }

        self.connections = conns;
    }

    fn refresh_data(&mut self) {
        self.loading = true;
        self.check_connections();

        // Reusar el runtime creado en new() — no crear uno nuevo.
        // Esto ahorra ~2MB de memoria por refresh y evita allocar/deallocar threads.
        if let Some(ref rt) = self.rt {
            let providers = all_providers();
            let mut results = Vec::with_capacity(providers.len());
            let mut errors = Vec::new();

            for p in providers {
                match rt.block_on(p.fetch()) {
                    Ok(usage) => results.push(usage),
                    Err(e) => errors.push((p.name().to_string(), e)),
                }
            }

            self.providers = results;
            self.errors = errors;
        }

        self.last_update = Some(std::time::Instant::now());
        self.loading = false;
    }

    /// Conectar un provider (abrir browser y preparar sync)
    fn connect_provider(&self, name: &str) {
        match name {
            "Claude" => {
                let _ = open::that("https://claude.ai/login");
            }
            "ChatGPT" => {
                let _ = open::that("https://chatgpt.com/auth/login");
            }
            "Antigravity" => {
                // Antigravity no tiene login web — el usuario abre la app
                // No hacemos nada, solo mostramos instrucciones
            }
            "OpenCode" => {
                let _ = open::that("https://opencode.ai");
            }
            "Kilo Code" => {
                let _ = open::that("https://kilocode.ai");
            }
            "Cursor" => {
                let _ = open::that("https://cursor.com");
            }
            _ => {}
        }
    }

    /// Sincronizar token después de login en browser
    fn sync_provider(&mut self, name: &str) -> String {
        // Re-chequear el archivo de credenciales
        match name {
            "Claude" => {
                let conn = check_claude_connection();
                if conn.status == AuthState::Connected {
                    self.disconnected.remove("Claude");
                    self.check_connections();
                    format!("Connected! {}", conn.detail)
                } else {
                    format!("Token not found. Please login first, then click Sync.")
                }
            }
            "ChatGPT" => {
                let conn = check_chatgpt_connection();
                if conn.status == AuthState::Connected {
                    self.disconnected.remove("ChatGPT");
                    self.check_connections();
                    format!("Connected! {}", conn.detail)
                } else {
                    "Token not found. Please login first, then click Sync.".to_string()
                }
            }
            "Antigravity" => {
                let conn = check_antigravity_connection();
                self.check_connections();
                conn.detail
            }
            "OpenCode" => {
                let conn = check_opencode_connection();
                self.check_connections();
                conn.detail
            }
            "Kilo Code" => {
                let conn = check_kilocode_connection();
                self.check_connections();
                conn.detail
            }
            "Cursor" => {
                let conn = check_cursor_connection();
                self.check_connections();
                conn.detail
            }
            _ => "Unknown provider".to_string(),
        }
    }

    /// Desconectar un provider
    fn disconnect_provider(&mut self, name: &str) {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();

        match name {
            "Claude" => {
                let cred_path = std::path::Path::new(&home)
                    .join(".claude")
                    .join(".credentials.json");
                if cred_path.exists() {
                    // No borramos — solo "desconectamos" lógicamente.
                    // Borrar el archivo rompería Claude CLI.
                    // En su lugar, limpiamos el access_token del JSON.
                    if let Ok(content) = std::fs::read_to_string(&cred_path) {
                        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(oauth) = json.get_mut("claudeAiOauth") {
                                oauth["accessToken"] = serde_json::Value::String("".to_string());
                            }
                            let _ = std::fs::write(
                                &cred_path,
                                serde_json::to_string_pretty(&json).unwrap(),
                            );
                        }
                    }
                }
                self.disconnected.insert("Claude".to_string());
            }
            "ChatGPT" => {
                let auth_path = std::path::Path::new(&home).join(".codex").join("auth.json");
                if auth_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&auth_path) {
                        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(tokens) = json.get_mut("tokens") {
                                tokens["access_token"] = serde_json::Value::String("".to_string());
                            }
                            let _ = std::fs::write(
                                &auth_path,
                                serde_json::to_string_pretty(&json).unwrap(),
                            );
                        }
                    }
                }
                self.disconnected.insert("ChatGPT".to_string());
            }
            _ => {
                self.disconnected.insert(name.to_string());
            }
        }

        self.check_connections();
    }
}

// --- EFRAME APP -------------------------------------------------------------
impl eframe::App for UsageTrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(last) = self.last_update {
            if last.elapsed().as_secs() >= self.refresh_secs {
                self.refresh_data();
            }
        } else {
            self.refresh_data();
        }

        if self.compact {
            // --- MODO COMPACTO: sin tabs, solo providers ---
            self.show_compact(ctx);
        } else {
            // --- MODO COMPLETO: con tabs ---
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("🎯 Usage Tracker");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("⟳ Refresh").clicked() {
                            self.refresh_data();
                        }
                        if let Some(last) = self.last_update {
                            ui.label(format!("{}s ago", last.elapsed().as_secs()));
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.active_tab, Tab::Overview, "📊 Overview");
                    ui.selectable_value(&mut self.active_tab, Tab::Connections, "🔗 Connections");
                    ui.selectable_value(&mut self.active_tab, Tab::Settings, "⚙ Settings");
                });
            });

            egui::CentralPanel::default().show(ctx, |ui| match self.active_tab {
                Tab::Overview => self.show_overview(ui),
                Tab::Connections => self.show_connections(ui),
                Tab::Settings => self.show_settings(ui),
            });
        }

        // Solo repintamos cuando realmente hace falta:
        // - Si estamos cargando (para el spinner)
        // - Si estamos en Overview (para el timer "Updated Xs ago")
        // - Cada refresh_secs para chequear si toca auto-refresh
        //
        // Sin esto, egui NO redibuja — ahorra CPU y batería.
        if self.loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else if self.active_tab == Tab::Overview {
            // Repintar cada 1s para actualizar "Updated Xs ago"
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        } else {
            // En Connections/Settings, solo repintar cuando toca refresh
            if let Some(last) = self.last_update {
                let remaining = self.refresh_secs.saturating_sub(last.elapsed().as_secs());
                ctx.request_repaint_after(std::time::Duration::from_secs(remaining.max(1)));
            }
        }
    }
}

// --- COMPACT WIDGET ---------------------------------------------------------
impl UsageTrackerApp {
    fn show_compact(&mut self, ctx: &egui::Context) {
        // Sin top panel — solo el contenido, compacto
        egui::CentralPanel::default().show(ctx, |ui| {
            // Header mini
            ui.horizontal(|ui| {
                ui.strong("Usage");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("⟳").clicked() {
                        self.refresh_data();
                    }
                    if let Some(last) = self.last_update {
                        ui.label(
                            egui::RichText::new(format!("{}s", last.elapsed().as_secs())).small(),
                        );
                    }
                });
            });

            ui.separator();

            if self.loading {
                ui.vertical_centered(|ui| {
                    ui.spinner();
                });
                return;
            }

            // Mostrar providers compactos
            egui::ScrollArea::vertical().show(ui, |ui| {
                for provider in &self.providers {
                    self.show_compact_provider(ui, provider);
                    ui.add_space(2.0);
                }

                // Errores compactos
                for (name, err) in &self.errors {
                    ui.horizontal(|ui| {
                        let (logo, color) = provider_logo(name);
                        ui.colored_label(color, logo);
                        ui.colored_label(egui::Color32::RED, "✗");
                        let short = if err.len() > 20 {
                            format!("{}...", &err[..20])
                        } else {
                            err.clone()
                        };
                        ui.label(
                            egui::RichText::new(short)
                                .small()
                                .color(egui::Color32::from_rgb(255, 100, 100)),
                        );
                    });
                }
            });
        });
    }

    fn show_compact_provider(&self, ui: &mut egui::Ui, provider: &ProviderUsage) {
        ui.horizontal(|ui| {
            let (logo, color) = provider_logo(&provider.name);
            ui.colored_label(color, logo);
            ui.label(egui::RichText::new(&provider.name).strong().size(12.0));

            // Mostrar el mayor porcentaje a la derecha
            let max_pct = provider.session_pct.or(provider.weekly_pct);
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| match max_pct {
                    Some(pct) => {
                        let color = pct_color_compact(pct);
                        ui.colored_label(color, format!("{}%", pct));
                    }
                    None => {
                        ui.colored_label(egui::Color32::GRAY, "N/A");
                    }
                },
            );
        });

        // Barra de progreso mini
        if let Some(pct) = provider.session_pct {
            ui.add(
                egui::ProgressBar::new(pct as f32 / 100.0)
                    .desired_width(ui.available_width())
                    .fill(pct_color_compact(pct)),
            );
        }
    }
}

// --- OVERVIEW TAB -----------------------------------------------------------
impl UsageTrackerApp {
    fn show_overview(&mut self, ui: &mut egui::Ui) {
        if self.loading {
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.label("Loading...");
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (name, err) in &self.errors {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::RED, "✗");
                    ui.colored_label(egui::Color32::WHITE, name);
                    let short_err = if err.len() > 80 {
                        format!("{}...", &err[..80])
                    } else {
                        err.clone()
                    };
                    ui.colored_label(egui::Color32::from_rgb(255, 100, 100), short_err);
                });
            }

            ui.add_space(10.0);

            // Iteramos por referencia (&providers), no clonamos.
            // clone() en cada frame es un desperdicio de memoria.
            for provider in &self.providers {
                self.show_provider_card(ui, provider);
                ui.add_space(8.0);
            }

            if self.providers.is_empty() && self.errors.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.label("No data available.");
                    ui.label("Click Refresh or go to Connections tab.");
                });
            }
        });
    }

    fn show_provider_card(&self, ui: &mut egui::Ui, provider: &ProviderUsage) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
            .corner_radius(8)
            .inner_margin(12.0)
            .show(ui, |ui| {
                // Header: logo + nombre a la izq, reset a la der
                ui.horizontal(|ui| {
                    // Logo del provider
                    let (logo, logo_color) = provider_logo(&provider.name);
                    ui.colored_label(logo_color, logo);
                    ui.strong(&provider.name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("⏱ {}", provider.reset_time));
                    });
                });

                ui.add_space(4.0);

                // Barras: usan el ancho DISPONIBLE menos el label.
                // desired_width se adapta al espacio real.
                // min_width(80.0) evita que desaparezcan, pero se achican si hace falta.
                let label_width = 65.0; // ancho aproximado de "Session:" / "Weekly: "
                let bar_width = (ui.available_width() - label_width).max(80.0);

                if let Some(pct) = provider.session_pct {
                    ui.horizontal(|ui| {
                        ui.label("Session:");
                        ui.add(
                            egui::ProgressBar::new(pct as f32 / 100.0)
                                .desired_width(bar_width)
                                .fill(pct_color(pct))
                                .text(format!("{}%", pct)),
                        );
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Session:");
                        ui.colored_label(egui::Color32::GRAY, "N/A");
                    });
                }

                if let Some(pct) = provider.weekly_pct {
                    ui.horizontal(|ui| {
                        ui.label("Weekly: ");
                        ui.add(
                            egui::ProgressBar::new(pct as f32 / 100.0)
                                .desired_width(bar_width)
                                .fill(pct_color(pct))
                                .text(format!("{}%", pct)),
                        );
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Weekly: ");
                        ui.colored_label(egui::Color32::GRAY, "N/A");
                    });
                }
            });
    }

    // --- CONNECTIONS TAB ----------------------------------------------------
    fn show_connections(&mut self, ui: &mut egui::Ui) {
        ui.heading("Account Connections");
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for i in 0..self.connections.len() {
                // Clonamos para evitar borrow issues
                let conn = self.connections[i].clone();
                self.show_connection_card(ui, &conn, i);
                ui.add_space(10.0);
            }

            ui.separator();
            if ui.button("🔄 Re-check All Connections").clicked() {
                self.check_connections();
            }
        });
    }

    fn show_connection_card(&mut self, ui: &mut egui::Ui, conn: &ConnectionStatus, idx: usize) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(
                1.0,
                match conn.status {
                    AuthState::Connected => egui::Color32::from_rgb(76, 175, 80),
                    AuthState::Expired => egui::Color32::from_rgb(255, 193, 7),
                    AuthState::NotConfigured => egui::Color32::from_gray(60),
                },
            ))
            .corner_radius(8)
            .inner_margin(12.0)
            .show(ui, |ui| {
                // Header: name + status
                ui.horizontal(|ui| {
                    let (icon, color) = match conn.status {
                        AuthState::Connected => ("●", egui::Color32::from_rgb(76, 175, 80)),
                        AuthState::Expired => ("●", egui::Color32::from_rgb(255, 193, 7)),
                        AuthState::NotConfigured => ("○", egui::Color32::GRAY),
                    };
                    ui.colored_label(color, icon);
                    ui.strong(&conn.name);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let status_text = match conn.status {
                            AuthState::Connected => "Connected",
                            AuthState::Expired => "Expired",
                            AuthState::NotConfigured => "Not connected",
                        };
                        ui.label(status_text);
                    });
                });

                // Email / detail
                ui.add_space(4.0);
                if let Some(email) = &conn.email {
                    ui.label(format!("📧 {}", email));
                }
                ui.label(&conn.detail);

                // Action buttons
                ui.add_space(8.0);

                match conn.status {
                    AuthState::Connected => {
                        if ui.button("🚪 Disconnect").clicked() {
                            self.disconnect_provider(&conn.name);
                        }
                    }
                    AuthState::Expired | AuthState::NotConfigured => {
                        self.show_auth_actions(ui, conn, idx);
                    }
                }
            });
    }

    /// Mostrar acciones de auth según el provider
    fn show_auth_actions(&mut self, ui: &mut egui::Ui, conn: &ConnectionStatus, idx: usize) {
        match conn.name.as_str() {
            "Claude" => {
                ui.horizontal(|ui| {
                    if ui.button("🌐 Open Login").clicked() {
                        self.connect_provider("Claude");
                    }
                    if ui.button("🔄 Sync Token").clicked() {
                        self.connections[idx].detail = self.sync_provider("Claude");
                    }
                    // ℹ️ info tooltip
                    ui.label("ℹ").on_hover_text(
                        "1. Click 'Open Login' to authenticate in browser\n\
                         2. After login, click 'Sync Token'\n\n\
                         If that doesn't work, run in terminal:\n\
                         claude login",
                    );
                });

                // Fallback: botón que abre terminal con claude login
                ui.horizontal(|ui| {
                    ui.label("  ");
                    if ui.button("💻 Run 'claude login' in terminal").clicked() {
                        self.run_claude_login();
                    }
                });
            }
            "ChatGPT" => {
                ui.horizontal(|ui| {
                    if ui.button("🌐 Open Login").clicked() {
                        self.connect_provider("ChatGPT");
                    }
                    if ui.button("🔄 Sync Token").clicked() {
                        self.connections[idx].detail = self.sync_provider("ChatGPT");
                    }
                    ui.label("ℹ").on_hover_text(
                        "ChatGPT auth comes from Codex CLI.\n\
                         Run 'codex login' in terminal if sync fails.",
                    );
                });
            }
            "Antigravity" => {
                ui.horizontal(|ui| {
                    ui.label("Open Antigravity or Windsurf app.");
                    if ui.button("🔄 Check").clicked() {
                        self.connections[idx].detail = self.sync_provider("Antigravity");
                    }
                    ui.label("ℹ").on_hover_text(
                        "Antigravity doesn't have a web login.\n\
                         Just open the app — the tracker detects it automatically.",
                    );
                });
            }
            "OpenCode" => {
                ui.horizontal(|ui| {
                    if ui.button("🌐 Website").clicked() {
                        self.connect_provider("OpenCode");
                    }
                    if ui.button("🔄 Check").clicked() {
                        self.connections[idx].detail = self.sync_provider("OpenCode");
                    }
                });
            }
            "Kilo Code" => {
                ui.horizontal(|ui| {
                    if ui.button("🌐 Website").clicked() {
                        self.connect_provider("OpenCode");
                    }
                    ui.label("ℹ").on_hover_text(
                        "Kilo Code is a VS Code extension.\n\
                         No usage API available — just detect installation.",
                    );
                });
            }
            "Cursor" => {
                ui.horizontal(|ui| {
                    if ui.button("🌐 Website").clicked() {
                        let _ = open::that("https://cursor.com");
                    }
                    ui.label("ℹ").on_hover_text(
                        "Cursor auth is managed inside the app.\n\
                         No external token to sync.",
                    );
                });
            }
            _ => {}
        }
    }

    /// Ejecutar `claude login` en una terminal
    fn run_claude_login(&self) {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();

        let claude_dir = std::path::Path::new(&home).join(".claude");

        if cfg!(target_os = "windows") {
            // Abrir CMD en la carpeta de Claude y ejecutar claude login
            let _ = std::process::Command::new("cmd")
                .args([
                    "/C",
                    "start",
                    "cmd",
                    "/K",
                    &format!("cd /d {} && claude login", claude_dir.display()),
                ])
                .spawn();
        } else {
            // Linux/Mac: abrir terminal
            let _ = std::process::Command::new("x-terminal-emulator")
                .args([
                    "-e",
                    "bash",
                    "-c",
                    &format!("cd {} && claude login; exec bash", claude_dir.display()),
                ])
                .spawn()
                .or_else(|_| {
                    std::process::Command::new("gnome-terminal")
                        .args([
                            "--",
                            "bash",
                            "-c",
                            &format!("cd {} && claude login; exec bash", claude_dir.display()),
                        ])
                        .spawn()
                });
        }
    }

    // --- SETTINGS TAB -------------------------------------------------------
    fn show_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Auto-refresh interval (seconds):");
            ui.add(egui::DragValue::new(&mut self.refresh_secs).range(10..=3600));
        });

        ui.add_space(20.0);

        ui.heading("Config File");
        let config_path = dirs::config_dir()
            .map(|d| d.join("usage-tracker").join("config.toml"))
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "N/A".to_string());
        ui.monospace(&config_path);

        ui.add_space(20.0);

        ui.heading("About");
        ui.label("Usage Tracker v1.0.0");
        ui.label("Track AI usage: ChatGPT, Claude, Antigravity, OpenCode.");
    }
}

// --- CONNECTION CHECKERS ----------------------------------------------------

fn check_claude_connection() -> ConnectionStatus {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    let cred_path = std::path::Path::new(&home)
        .join(".claude")
        .join(".credentials.json");

    if !cred_path.exists() {
        return ConnectionStatus {
            name: "Claude".to_string(),
            status: AuthState::NotConfigured,
            detail: "Claude CLI not installed".to_string(),
            email: None,
        };
    }

    match std::fs::read_to_string(&cred_path) {
        Ok(content) => {
            #[derive(Deserialize)]
            struct Creds {
                #[serde(rename = "claudeAiOauth")]
                oauth: Option<OAuth>,
            }
            #[derive(Deserialize)]
            struct OAuth {
                #[serde(rename = "accessToken")]
                #[allow(dead_code)]
                access_token: Option<String>,
                #[serde(rename = "expiresAt")]
                expires_at: Option<u64>,
            }

            match serde_json::from_str::<Creds>(&content) {
                Ok(creds) => {
                    if let Some(oauth) = creds.oauth {
                        if let Some(exp) = oauth.expires_at {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64;

                            // Check if token is empty (disconnected)
                            let token_empty = oauth
                                .access_token
                                .as_ref()
                                .map(|t| t.is_empty())
                                .unwrap_or(true);

                            if token_empty {
                                ConnectionStatus {
                                    name: "Claude".to_string(),
                                    status: AuthState::NotConfigured,
                                    detail: "Token cleared. Login required.".to_string(),
                                    email: None,
                                }
                            } else if exp > now {
                                let hours = (exp - now) / 3600000;
                                ConnectionStatus {
                                    name: "Claude".to_string(),
                                    status: AuthState::Connected,
                                    detail: format!("Token valid — {}h remaining", hours),
                                    email: None,
                                }
                            } else {
                                ConnectionStatus {
                                    name: "Claude".to_string(),
                                    status: AuthState::Expired,
                                    detail: "Token expired — re-login required".to_string(),
                                    email: None,
                                }
                            }
                        } else {
                            ConnectionStatus {
                                name: "Claude".to_string(),
                                status: AuthState::NotConfigured,
                                detail: "No expiresAt in token".to_string(),
                                email: None,
                            }
                        }
                    } else {
                        ConnectionStatus {
                            name: "Claude".to_string(),
                            status: AuthState::NotConfigured,
                            detail: "No OAuth data in credentials".to_string(),
                            email: None,
                        }
                    }
                }
                Err(e) => ConnectionStatus {
                    name: "Claude".to_string(),
                    status: AuthState::NotConfigured,
                    detail: format!("Cannot parse credentials: {}", e),
                    email: None,
                },
            }
        }
        Err(e) => ConnectionStatus {
            name: "Claude".to_string(),
            status: AuthState::NotConfigured,
            detail: format!("Cannot read credentials: {}", e),
            email: None,
        },
    }
}

fn check_chatgpt_connection() -> ConnectionStatus {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    let auth_path = std::path::Path::new(&home).join(".codex").join("auth.json");

    if !auth_path.exists() {
        return ConnectionStatus {
            name: "ChatGPT".to_string(),
            status: AuthState::NotConfigured,
            detail: "Codex CLI not installed".to_string(),
            email: None,
        };
    }

    match std::fs::read_to_string(&auth_path) {
        Ok(content) => {
            #[derive(Deserialize)]
            struct Auth {
                tokens: Option<Tokens>,
            }
            #[derive(Deserialize)]
            struct Tokens {
                access_token: Option<String>,
                account_id: Option<String>,
            }

            match serde_json::from_str::<Auth>(&content) {
                Ok(auth) => {
                    if let Some(tokens) = auth.tokens {
                        let token = tokens.access_token.unwrap_or_default();
                        if token.is_empty() {
                            ConnectionStatus {
                                name: "ChatGPT".to_string(),
                                status: AuthState::NotConfigured,
                                detail: "Token cleared. Login required.".to_string(),
                                email: tokens.account_id,
                            }
                        } else {
                            ConnectionStatus {
                                name: "ChatGPT".to_string(),
                                status: AuthState::Connected,
                                detail: "Token active".to_string(),
                                email: tokens.account_id,
                            }
                        }
                    } else {
                        ConnectionStatus {
                            name: "ChatGPT".to_string(),
                            status: AuthState::NotConfigured,
                            detail: "No tokens in auth.json".to_string(),
                            email: None,
                        }
                    }
                }
                Err(e) => ConnectionStatus {
                    name: "ChatGPT".to_string(),
                    status: AuthState::NotConfigured,
                    detail: format!("Cannot parse auth.json: {}", e),
                    email: None,
                },
            }
        }
        Err(e) => ConnectionStatus {
            name: "ChatGPT".to_string(),
            status: AuthState::NotConfigured,
            detail: format!("Cannot read auth.json: {}", e),
            email: None,
        },
    }
}

fn check_antigravity_connection() -> ConnectionStatus {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq language_server*", "/FO", "CSV"])
            .output()
            .ok()
    } else {
        std::process::Command::new("ps")
            .args(["-ax", "-o", "command="])
            .output()
            .ok()
    };

    let running = output
        .map(|out| String::from_utf8_lossy(&out.stdout).contains("language_server"))
        .unwrap_or(false);

    if running {
        ConnectionStatus {
            name: "Antigravity".to_string(),
            status: AuthState::Connected,
            detail: "Language server running".to_string(),
            email: None,
        }
    } else {
        ConnectionStatus {
            name: "Antigravity".to_string(),
            status: AuthState::NotConfigured,
            detail: "Language server not detected".to_string(),
            email: None,
        }
    }
}

fn check_opencode_connection() -> ConnectionStatus {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    let config_path = std::path::Path::new(&home)
        .join(".config")
        .join("opencode")
        .join("opencode.json");

    let alt_path = std::path::Path::new(&home)
        .join(".opencode")
        .join("opencode.json");

    if config_path.exists() || alt_path.exists() {
        ConnectionStatus {
            name: "OpenCode".to_string(),
            status: AuthState::Connected,
            detail: "Config found (no usage API)".to_string(),
            email: None,
        }
    } else {
        ConnectionStatus {
            name: "OpenCode".to_string(),
            status: AuthState::NotConfigured,
            detail: "OpenCode not installed".to_string(),
            email: None,
        }
    }
}

// --- HELPERS ----------------------------------------------------------------

fn pct_color(pct: i32) -> egui::Color32 {
    match pct {
        0..=49 => egui::Color32::from_rgb(76, 175, 80),
        50..=79 => egui::Color32::from_rgb(255, 193, 7),
        _ => egui::Color32::from_rgb(244, 67, 54),
    }
}

fn pct_color_compact(pct: i32) -> egui::Color32 {
    pct_color(pct)
}

fn configure_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_corner_radius = egui::CornerRadius::same(10);
    visuals.panel_fill = egui::Color32::from_rgb(30, 30, 30);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(18.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(14.0, egui::FontFamily::Proportional),
    );
    ctx.set_style(style);
}

// --- PROVIDER LOGOS ---------------------------------------------------------
//
// Cada provider tiene un "logo" Unicode con un color distintivo.
// No usamos imágenes — son caracteres Unicode coloreados.
// En egui, no podemos embeber PNGs fácilmente, así que usamos texto estilizado.
fn provider_logo(name: &str) -> (&'static str, egui::Color32) {
    match name {
        "Claude" => ("◆", egui::Color32::from_rgb(204, 147, 107)), // naranja cálido (Anthropic)
        "ChatGPT" => ("●", egui::Color32::from_rgb(16, 163, 127)), // verde (OpenAI)
        "Antigravity" => ("▲", egui::Color32::from_rgb(124, 77, 255)), // violeta (Windsurf)
        "Kilo Code" => ("■", egui::Color32::from_rgb(255, 152, 0)), // ámbar
        "Cursor" => ("│", egui::Color32::from_rgb(200, 200, 200)), // gris claro
        "OpenCode" => ("◇", egui::Color32::from_rgb(0, 188, 212)), // cyan
        _ => ("•", egui::Color32::GRAY),
    }
}

// --- CONNECTION CHECKERS: KILO CODE + CURSOR --------------------------------

fn check_kilocode_connection() -> ConnectionStatus {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    let kilo_dir = std::path::Path::new(&home).join(".config").join("kilo");
    let alt_dir = std::path::Path::new(&home).join(".kilocode");

    if kilo_dir.exists() || alt_dir.exists() {
        ConnectionStatus {
            name: "Kilo Code".to_string(),
            status: AuthState::Connected,
            detail: "Config found (no usage API available)".to_string(),
            email: None,
        }
    } else {
        ConnectionStatus {
            name: "Kilo Code".to_string(),
            status: AuthState::NotConfigured,
            detail: "Kilo Code not installed".to_string(),
            email: None,
        }
    }
}

fn check_cursor_connection() -> ConnectionStatus {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    let cursor_dir = std::path::Path::new(&home).join(".cursor");

    if cursor_dir.exists() {
        ConnectionStatus {
            name: "Cursor".to_string(),
            status: AuthState::Connected,
            detail: "Config found (no usage API available)".to_string(),
            email: None,
        }
    } else {
        ConnectionStatus {
            name: "Cursor".to_string(),
            status: AuthState::NotConfigured,
            detail: "Cursor not installed".to_string(),
            email: None,
        }
    }
}
