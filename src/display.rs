// =============================================================================
// display.rs — Formateo de output (tablas + colores)
// =============================================================================
//
// Conceptos nuevos:
//   - comfy_table::Table → tablas con bordes Unicode
//   - colored::Colorize → colores en terminal (.red(), .green(), etc.)
//   - Separar display de lógica → display.rs solo sabe MOSTRAR
// =============================================================================

use colored::Colorize;
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use crate::provider::ProviderUsage;

// --- MOSTRAR TABLA DE PROVIDERS ---------------------------------------------
//
// Recibe un slice de ProviderUsage (&[ProviderUsage]) y muestra una tabla.
//
// ¿Qué es un slice &[T]?
//   Es una "vista" de un array/vector. No es dueño de los datos, solo los referencia.
//   Como un &str pero para arrays. Más eficiente que pasar Vec<T> (no copia).
pub fn display_table(providers: &[ProviderUsage]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Provider").fg(Color::Cyan),
            Cell::new("Session").fg(Color::Cyan),
            Cell::new("Weekly").fg(Color::Cyan),
            Cell::new("Reset").fg(Color::Cyan),
        ]);

    for p in providers {
        let session_str = format_pct(p.session_pct);
        let weekly_str = format_pct(p.weekly_pct);

        // Color de la fila basado en el mayor porcentaje
        let max_pct = p.session_pct.or(p.weekly_pct).unwrap_or(0);
        let row_color = pct_color(max_pct);

        table.add_row(vec![
            Cell::new(&p.name).fg(Color::White),
            Cell::new(&session_str).fg(row_color),
            Cell::new(&weekly_str).fg(row_color),
            Cell::new(&p.reset_time).fg(Color::Yellow),
        ]);
    }

    println!("{}", table);
}

// --- MOSTRAR ERROR DE PROVIDER ----------------------------------------------
pub fn display_error(name: &str, error: &str) {
    // cortar errores muy largos
    let short_err = if error.len() > 120 {
        format!("{}...", &error[..120])
    } else {
        error.to_string()
    };
    println!(
        "  {} {}: {}",
        "✗".red().bold(),
        name.white(),
        short_err.red()
    );
}

// --- FORMATO JSON -----------------------------------------------------------
pub fn display_json(providers: &[ProviderUsage]) {
    // serde_json::to_string_pretty convierte un slice de structs a JSON bonito.
    // Para que funcione, ProviderUsage necesita #[derive(Serialize)].
    // Lo agregamos en provider.rs.
    match serde_json::to_string_pretty(providers) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error serializando JSON: {}", e),
    }
}

// --- HELPERS ----------------------------------------------------------------

fn format_pct(pct: Option<i32>) -> String {
    match pct {
        Some(p) => {
            let s = format!("{}%", p);
            // Coloreamos el texto según el valor
            match p {
                0..=49 => s.green().to_string(),
                50..=79 => s.yellow().to_string(),
                80..=100 => s.red().bold().to_string(),
                _ => s,
            }
        }
        None => "N/A".dimmed().to_string(),
    }
}

fn pct_color(pct: i32) -> Color {
    match pct {
        0..=49 => Color::Green,
        50..=79 => Color::Yellow,
        _ => Color::Red,
    }
}
