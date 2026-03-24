// =============================================================================
// provider.rs — Tipos compartidos: ProviderUsage + trait Provider (async)
// =============================================================================
//
// CAMBIO de Phase 2 → Phase 3:
//   - Se agrega #[async_trait] al trait
//   - fetch() ahora es async fn fetch()
//
// ¿Por qué async?
//   Porque hacer una llamada HTTP es I/O (entrada/salida).
//   Sin async, el programa se BLOQUEA esperando la respuesta del servidor.
//   Con async, puede hacer otras cosas mientras espera (o en nuestro caso,
//   esperar de forma eficiente sin quemar CPU).
//
// ¿Qué es #[async_trait]?
//   En Rust estable, no podés escribir "async fn" dentro de un trait directamente.
//   async-trait es un crate que resuelve esto con un macro.
//   Bajo el capó, convierte `async fn fetch()` en algo como:
//     fn fetch(&self) -> Pin<Box<dyn Future<Output = Result<...>> + Send>>
//   No te preocupes por eso ahora. Solo sabé que #[async_trait] lo hace funcionar.
// =============================================================================

use async_trait::async_trait;
use serde::Serialize;

// --- STRUCT: PROVIDER_USAGE (sin cambios de Phase 2) -----------------------
//
// pub = público. Otros módulos pueden acceder a estos campos.
// Sin pub = privado. Solo el módulo actual puede usarlos.
//
// ¿Por qué pub? Porque main.rs necesita leer estos campos para mostrarlos.
// ¿Por qué Option<i32> para los porcentajes?
//   Porque un provider puede no tener datos (como OpenCode en Phase 1).
//   Option<T> es Rust's way de decir "puede no existir".
#[derive(Debug, Serialize, Clone)]
pub struct ProviderUsage {
    pub name: String,
    pub session_pct: Option<i32>,
    pub weekly_pct: Option<i32>,
    pub reset_time: String,
}

// --- TRAIT: PROVIDER (ahora async) -----------------------------------------
//
// #[async_trait] = macro que transforma async fn en algo que el compilador acepta.
// Sin esto, obtendrías un error: "async fn is not permitted in a trait"
//
// CAMBIO: fetch() ahora es async fn.
// Esto permite que cada provider haga HTTP, lea archivos, etc. sin bloquear.
#[async_trait]
pub trait Provider {
    fn name(&self) -> &str;

    // async fn = función asíncrona. Retorna un Future (promesa, en JS terms).
    // El caller tiene que hacer .await para obtener el resultado.
    async fn fetch(&self) -> Result<ProviderUsage, String>;
}
