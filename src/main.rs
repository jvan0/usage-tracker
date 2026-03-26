// =============================================================================
// usage-tracker — main.rs
// =============================================================================
//
// Subcomandos:
//   check    → mostrar usage una vez (CLI)
//   watch    → mostrar usage cada N segundos (CLI auto-refresh)
//   gui      → abrir app de escritorio con egui
//   init     → crear config.toml de ejemplo
// =============================================================================

// En release de Windows, usar subsystem "windows" en vez de "console".
// Esto EVITA que se abra una terminal al hacer doble click en el .exe.
// En debug mantenemos la consola para poder ver errores/println.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use clap::{Parser, Subcommand};

mod config;
mod display;
mod gui;
mod provider;
mod providers;
mod tray;

use providers::{all_providers, get_provider};

#[derive(Parser)]
#[command(
    name = "usage-tracker",
    version = "1.0.1",
    about = "Track AI provider usage across ChatGPT, Claude, Antigravity, and OpenCode"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Check current usage for one or all providers
    Check(CheckArgs),

    /// Watch usage with auto-refresh
    Watch(WatchArgs),

    /// Open desktop GUI
    Gui,

    /// Open compact widget (always on top)
    Widget,

    /// Start system tray (controls the widget)
    Tray,

    /// Add to Windows startup (auto-launch)
    Install,

    /// Remove from Windows startup
    Uninstall,

    /// Create default config file
    Init,
}

#[derive(clap::Args)]
struct CheckArgs {
    #[arg(short, long, default_value = "all")]
    provider: ProviderChoice,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct WatchArgs {
    /// Refresh interval in seconds (overrides config)
    #[arg(short, long)]
    interval: Option<u64>,

    /// Only watch specific provider
    #[arg(short, long)]
    provider: Option<ProviderChoice>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum ProviderChoice {
    Claude,
    Chatgpt,
    Antigravity,
    Kilocode,
    Cursor,
    Opencode,
    All,
}

fn main() {
    let args = Args::parse();

    match args.command {
        Some(Commands::Check(check_args)) => {
            let rt = tokio::runtime::Runtime::new().expect("Error creating tokio runtime");
            rt.block_on(handle_check(check_args));
        }
        Some(Commands::Watch(watch_args)) => {
            let rt = tokio::runtime::Runtime::new().expect("Error creating tokio runtime");
            rt.block_on(handle_watch(watch_args));
        }
        Some(Commands::Gui) => handle_gui(),
        Some(Commands::Widget) => handle_widget(),
        Some(Commands::Tray) => tray::run_tray(),
        Some(Commands::Install) => handle_install(),
        Some(Commands::Uninstall) => handle_uninstall(),
        Some(Commands::Init) => handle_init(),
        None => handle_gui(),
    }
}

// --- CHECK: mostrar una vez -------------------------------------------------
async fn handle_check(args: CheckArgs) {
    let cfg = config::load_config();
    let providers = get_providers_filtered(&args.provider, &cfg);

    let mut results: Vec<provider::ProviderUsage> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();

    for p in providers {
        match p.fetch().await {
            Ok(usage) => results.push(usage),
            Err(err) => errors.push((p.name().to_string(), err)),
        }
    }

    if args.json {
        display::display_json(&results);
    } else {
        for (name, err) in &errors {
            display::display_error(name, err);
        }
        if !results.is_empty() {
            display::display_table(&results);
        }
    }
}

// --- WATCH: auto-refresh loop ----------------------------------------------
//
// Concepto nuevo: tokio::time::interval
// Crea un "ticker" que dispara cada N segundos.
// .tick().await espera hasta el próximo tick.
//
// Es como setInterval() en JavaScript, pero async.
async fn handle_watch(args: WatchArgs) {
    let cfg = config::load_config();
    let interval_secs = args.interval.unwrap_or(cfg.refresh_secs);

    println!("Watching every {}s. Press Ctrl+C to stop.\n", interval_secs);

    // tokio::time::interval crea un Interval que dispara cada duración.
    let mut interval = tokio::time::interval(
        std::time::Duration::from_secs(interval_secs)
    );

    loop {
        // .tick().await espera al próximo tick
        interval.tick().await;

        // Limpiar pantalla (ANSI escape code)
        print!("\x1B[2J\x1B[1;1H");

        // Timestamp
        let now = chrono_now();
        println!("{} Last update: {}\n", "⟳".cyan().bold(), now.dimmed());

        // Fetch y mostrar
        let provider_choice = args.provider.as_ref().unwrap_or(&ProviderChoice::All);
        let providers = get_providers_filtered(provider_choice, &cfg);

        let mut results: Vec<provider::ProviderUsage> = Vec::new();
        let mut errors: Vec<(String, String)> = Vec::new();

        for p in providers {
            match p.fetch().await {
                Ok(usage) => results.push(usage),
                Err(err) => errors.push((p.name().to_string(), err)),
            }
        }

        for (name, err) in &errors {
            display::display_error(name, err);
        }
        if !results.is_empty() {
            display::display_table(&results);
        }

        // Notificación si supera umbral
        if let Some(threshold) = cfg.notify_threshold {
            for r in &results {
                if let Some(pct) = r.session_pct.or(r.weekly_pct) {
                    if pct >= threshold {
                        eprintln!("⚠ {} usage at {}% — exceeds threshold of {}%",
                            r.name, pct, threshold);
                    }
                }
            }
        }
    }
}

// --- GUI: abrir app de escritorio -------------------------------------------
//
// eframe::run_native abre una ventana nativa con nuestra app egui.
// Le pasamos:
//   1. Nombre de la app (aparece en la barra de título)
//   2. Opciones de ventana (tamaño, etc.)
//   3. Un "creator" — closure que crea nuestra app
//
// ¿Qué es una closure?
//   Es una función anónima que puede capturar variables del scope exterior.
//   |cc| { ... } es una closure que recibe un parámetro "cc" (CreationContext).
//
//   Es como una arrow function en JS:
//     const creator = (cc) => new UsageTrackerApp(cc);
//   Pero en Rust:
//     Box::new(|cc| Ok(Box::new(gui::UsageTrackerApp::new(cc))))
fn handle_gui() {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([400.0, 500.0])
            .with_min_inner_size([350.0, 400.0])
            .with_title("Usage Tracker"),
        ..Default::default()
    };

    // run_native abre la ventana y corre el loop de UI.
    // No retorna hasta que el usuario cierra la ventana.
    eframe::run_native(
        "Usage Tracker",
        options,
        Box::new(|cc| Ok(Box::new(gui::UsageTrackerApp::new(cc)))),
    )
    .expect("Error al iniciar la GUI");
}

// --- INIT: crear config -----------------------------------------------------
fn handle_init() {
    match config::create_default_config() {
        Ok(path) => println!("Config created at: {:?}", path),
        Err(e) => eprintln!("Error: {}", e),
    }
}

// --- WIDGET: mini-ventana compacta -----------------------------------------
fn handle_widget() {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([280.0, 280.0])
            .with_min_inner_size([250.0, 200.0])
            .with_max_inner_size([400.0, 400.0])
            .with_always_on_top()
            .with_resizable(true)
            .with_title("Usage"),
        ..Default::default()
    };

    eframe::run_native(
        "Usage",
        options,
        Box::new(|cc| Ok(Box::new(gui::UsageTrackerApp::new_widget(cc)))),
    )
    .expect("Error al iniciar widget");
}

// --- INSTALL: auto-start en Windows ----------------------------------------
//
// Agrega el programa al registro de Windows para que arranque automáticamente.
// Usa la clave HKCU\Software\Microsoft\Windows\CurrentVersion\Run
fn handle_install() {
    if !cfg!(target_os = "windows") {
        println!("Auto-start solo soportado en Windows por ahora.");
        println!("En Linux/Mac, agregá el comando a tu .bashrc o crontab.");
        return;
    }

    let exe = std::env::current_exe()
        .expect("No se pudo obtener la ruta del ejecutable");
    let exe_str = exe.display().to_string();

    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let userprofile = std::env::var("USERPROFILE").unwrap_or_default();

    let mut ok = true;

    // 1. Acceso directo en Startup (auto-start)
    let startup_folder = format!("{}\\Microsoft\\Windows\\Start Menu\\Programs\\Startup", appdata);
    let startup_shortcut = format!("{}\\usage-tracker.lnk", startup_folder);

    let ps_startup = format!(
        r#"$ws = New-Object -ComObject WScript.Shell
$sc = $ws.CreateShortcut('{}')
$sc.TargetPath = '{}'
$sc.Arguments = 'tray'
$sc.Description = 'AI Usage Tracker - System Tray'
$sc.Save()"#,
        startup_shortcut, exe_str
    );

    match std::process::Command::new("powershell").args(["-Command", &ps_startup]).output() {
        Ok(out) if out.status.success() => {
            println!("✅ Auto-start: {}", startup_shortcut);
        }
        _ => {
            eprintln!("⚠ No se pudo crear acceso directo en Startup.");
            ok = false;
        }
    }

    // 2. Acceso directo en Escritorio
    let desktop_folder = format!("{}\\Desktop", userprofile);
    let desktop_shortcut = format!("{}\\Usage Tracker.lnk", desktop_folder);

    let ps_desktop = format!(
        r#"$ws = New-Object -ComObject WScript.Shell
$sc = $ws.CreateShortcut('{}')
$sc.TargetPath = '{}'
$sc.Arguments = 'gui'
$sc.Description = 'AI Usage Tracker'
$sc.Save()"#,
        desktop_shortcut, exe_str
    );

    match std::process::Command::new("powershell").args(["-Command", &ps_desktop]).output() {
        Ok(out) if out.status.success() => {
            println!("✅ Escritorio: {}", desktop_shortcut);
        }
        _ => {
            eprintln!("⚠ No se pudo crear acceso directo en el escritorio.");
            ok = false;
        }
    }

    if ok {
        println!("");
        println!("🎯 Usage Tracker instalado!");
        println!("   - Se abre automáticamente al iniciar Windows (tray)");
        println!("   - Acceso directo en el escritorio (GUI)");
    } else {
        println!("");
        println!("Alternativa manual:");
        println!("  1. Presioná Win+R");
        println!("  2. Escribí: shell:startup → pegá acceso directo para auto-start");
        println!("  3. Escribí: shell:desktop → pegá acceso directo para escritorio");
    }
}

// --- UNINSTALL: quitar auto-start -------------------------------------------
fn handle_uninstall() {
    if !cfg!(target_os = "windows") {
        println!("Auto-start solo soportado en Windows por ahora.");
        return;
    }

    let startup_folder = std::env::var("APPDATA")
        .map(|appdata| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs\\Startup", appdata))
        .unwrap_or_default();

    let shortcut_path = format!("{}\\usage-tracker.lnk", startup_folder);

    if std::path::Path::new(&shortcut_path).exists() {
        match std::fs::remove_file(&shortcut_path) {
            Ok(_) => println!("✅ Auto-start deshabilitado."),
            Err(e) => eprintln!("Error eliminando acceso directo: {}", e),
        }
    } else {
        println!("No hay auto-start configurado.");
    }
}

// --- HELPERS ----------------------------------------------------------------

fn get_providers_filtered(
    choice: &ProviderChoice,
    cfg: &config::AppConfig,
) -> Vec<Box<dyn provider::Provider>> {
    match choice {
        ProviderChoice::All => {
            // Si el config no especifica enabled_providers, mostrar todos.
            // Si especifica, filtrar.
            if cfg.enabled_providers.is_empty() {
                all_providers()
            } else {
                all_providers()
                    .into_iter()
                    .filter(|p| {
                        // Normalizar: "Kilo Code" → "kilocode", "ChatGPT" → "chatgpt"
                        let normalized = p.name().to_lowercase().replace(" ", "");
                        cfg.enabled_providers.contains(&normalized)
                    })
                    .collect()
            }
        }
        other => {
            let name = match other {
                ProviderChoice::Claude => "claude",
                ProviderChoice::Chatgpt => "chatgpt",
                ProviderChoice::Antigravity => "antigravity",
                ProviderChoice::Kilocode => "kilocode",
                ProviderChoice::Cursor => "cursor",
                ProviderChoice::Opencode => "opencode",
                ProviderChoice::All => unreachable!(),
            };
            get_provider(name)
                .map(|p| vec![p])
                .unwrap_or_default()
        }
    }
}

fn chrono_now() -> String {
    // Formato simple sin crate chrono: usar std::time
    // Para algo más lindo, agregar chrono crate
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02} UTC", hours, minutes, seconds)
}

// Colored re-imports para helpers
use colored::Colorize;
