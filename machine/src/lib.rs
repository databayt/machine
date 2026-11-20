//! The machine shell: loads kun's `/<lang>/machine` routes and registers every feature.
//! Feature code lives in `src/components/<feature>/rust.rs`, never here.

include!(concat!(env!("OUT_DIR"), "/features.rs"));

/// Starts the desktop app.
///
/// # Panics
/// If the Tauri runtime cannot start (no webview available).
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    register(tauri::Builder::default().plugin(tauri_plugin_updater::Builder::new().build()))
        .run(tauri::generate_context!())
        .expect("machine failed to start");
}
