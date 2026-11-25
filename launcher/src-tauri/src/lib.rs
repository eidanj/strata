//! Strata launcher backend. The React UI is presentation only; everything
//! that touches the disk or the network happens here.

mod auth;
mod content;
mod error;
mod hud;
mod import;
mod install;
mod instance;
mod launch;
mod modrinth;
mod paths;
mod profiles;
mod versions;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use content::{ContentItem, Kind};
use error::{Error, Result};
use paths::Paths;

pub struct AppState {
    pub paths: Paths,
    pub http: reqwest::Client,
    /// Serialises installs so two at once can't race on the lock file.
    pub install_lock: tokio::sync::Mutex<()>,
    pub auth: tokio::sync::Mutex<auth::AuthState>,
    pub game: tokio::sync::Mutex<Option<launch::GameHandle>>,
    pub launching: std::sync::atomic::AtomicBool,
    pub game_log: std::sync::Mutex<std::collections::VecDeque<String>>,
}

/// Run blocking file work off the async runtime's worker threads.
async fn blocking<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::msg(e.to_string()))?
}

// ----------------------------------------------------------------- overview

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    instance: instance::Instance,
    data_dir: String,
    iris_installed: bool,
}

#[tauri::command]
async fn get_overview(state: State<'_, AppState>) -> Result<Overview> {
    let paths = state.paths.clone();
    blocking(move || {
        let id = instance::active_id(&paths)?;
        let iris_installed = content::installed_mod_ids(&paths, &id).iter().any(|(m, _)| m == "iris");
        Ok(Overview {
            instance: instance::load(&paths, &id)?,
            data_dir: paths.root().display().to_string(),
            iris_installed,
        })
    })
    .await
}

// ------------------------------------------------------------------ content

#[tauri::command]
async fn list_content(state: State<'_, AppState>, kind: Kind) -> Result<Vec<ContentItem>> {
    let paths = state.paths.clone();
    blocking(move || {
        let id = instance::active_id(&paths)?;
        let inst = instance::load(&paths, &id)?;
        content::list(&paths, &id, kind, &inst.minecraft_version)
    })
    .await
}

#[tauri::command]
async fn set_mod_enabled(state: State<'_, AppState>, file: String, enabled: bool) -> Result<()> {
    let paths = state.paths.clone();
    blocking(move || {
        let id = instance::active_id(&paths)?;
        content::set_mod_enabled(&paths, &id, &file, enabled)
    })
    .await
}

/// `enabled` is the full list of enabled packs, highest priority first.
#[tauri::command]
async fn set_resource_packs(state: State<'_, AppState>, enabled: Vec<String>) -> Result<()> {
    let paths = state.paths.clone();
    blocking(move || {
        let id = instance::active_id(&paths)?;
        content::set_pack_order(&paths, &id, &enabled)
    })
    .await
}

#[tauri::command]
async fn set_active_shader(state: State<'_, AppState>, file: Option<String>) -> Result<()> {
    let paths = state.paths.clone();
    blocking(move || {
        let id = instance::active_id(&paths)?;
        content::set_active_shader(&paths, &id, file.as_deref())
    })
    .await
}

#[tauri::command]
async fn remove_content(state: State<'_, AppState>, kind: Kind, file: String) -> Result<()> {
    let paths = state.paths.clone();
    blocking(move || {
        let id = instance::active_id(&paths)?;
        content::remove(&paths, &id, kind, &file)
    })
    .await
}

#[tauri::command]
async fn open_content_folder(app: AppHandle, state: State<'_, AppState>, kind: Kind) -> Result<()> {
    let id = instance::active_id(&state.paths)?;
    let dir = state.paths.content_dir(&id, kind);
    std::fs::create_dir_all(&dir)?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| Error::msg(format!("Couldn't open the folder: {e}")))
}

/// Drag-and-drop and the "Add files" button both end up here.
#[tauri::command]
async fn import_files(state: State<'_, AppState>, kind: Kind, paths: Vec<String>) -> Result<import::ImportReport> {
    let app_paths = state.paths.clone();
    blocking(move || import::import(&app_paths, kind, &paths)).await
}

// ----------------------------------------------------------------- modrinth

#[tauri::command]
async fn modrinth_search(state: State<'_, AppState>, query: modrinth::SearchQuery) -> Result<modrinth::SearchResults> {
    let id = instance::active_id(&state.paths)?;
    let inst = instance::load(&state.paths, &id)?;
    modrinth::Modrinth::new(&state.http)
        .search(&query, &inst.minecraft_version, &inst.loader)
        .await
}

#[tauri::command]
async fn modrinth_install(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: Kind,
    project: String,
) -> Result<install::InstallReport> {
    install::install(&app, &state, kind, &project).await
}

// ---------------------------------------------------------- HUD + profiles

#[tauri::command]
async fn get_hud_config(state: State<'_, AppState>) -> Result<Value> {
    let paths = state.paths.clone();
    blocking(move || hud::load(&paths)).await
}

#[tauri::command]
async fn save_hud_config(state: State<'_, AppState>, config: Value) -> Result<()> {
    let paths = state.paths.clone();
    blocking(move || hud::save(&paths, config)).await
}

#[tauri::command]
fn default_hud_config() -> Value {
    hud::default_config()
}

#[tauri::command]
async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Value>> {
    let paths = state.paths.clone();
    blocking(move || profiles::list(&paths)).await
}

#[tauri::command]
async fn set_active_profile(state: State<'_, AppState>, profile: String) -> Result<()> {
    let paths = state.paths.clone();
    blocking(move || profiles::set_active(&paths, &profile)).await
}

// ------------------------------------------------------------ accounts

#[tauri::command]
async fn get_accounts(state: State<'_, AppState>) -> Result<auth::AccountsView> {
    let paths = state.paths.clone();
    blocking(move || auth::view(&paths)).await
}

#[tauri::command]
async fn set_client_id(state: State<'_, AppState>, id: String) -> Result<()> {
    auth::set_client_id(&state.paths, &id)
}

#[tauri::command]
async fn auth_begin(app: AppHandle, state: State<'_, AppState>) -> Result<auth::DeviceCode> {
    auth::begin(app.clone(), &state).await
}

#[tauri::command]
async fn auth_cancel(state: State<'_, AppState>) -> Result<()> {
    auth::cancel(&state).await;
    Ok(())
}

/// Opens the URL Microsoft gave us; the frontend can't pass its own.
#[tauri::command]
async fn auth_open_link(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let url = auth::verification_uri(&state).await.ok_or_else(|| Error::msg("No sign-in in progress."))?;
    app.opener().open_url(url, None::<&str>).map_err(|e| Error::msg(format!("Couldn't open the browser: {e}")))
}

#[tauri::command]
async fn select_account(state: State<'_, AppState>, uuid: String) -> Result<()> {
    auth::select(&state.paths, &uuid)
}

#[tauri::command]
async fn sign_out(state: State<'_, AppState>, uuid: String) -> Result<()> {
    auth::sign_out(&state, &uuid).await
}

#[tauri::command]
async fn add_test_account(state: State<'_, AppState>, name: String) -> Result<()> {
    auth::add_test_account(&state.paths, &name)
}

// ------------------------------------------------------------- launching

#[tauri::command]
async fn list_versions(state: State<'_, AppState>) -> Result<Vec<launch::VersionOption>> {
    launch::versions(&state).await
}

#[tauri::command]
async fn set_version(state: State<'_, AppState>, version: String) -> Result<instance::Instance> {
    launch::set_version(&state, &version).await
}

#[tauri::command]
async fn launch_game(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    launch::launch(app.clone(), &state).await
}

#[tauri::command]
async fn stop_game(state: State<'_, AppState>) -> Result<()> {
    launch::stop(&state).await
}

#[tauri::command]
async fn game_status(state: State<'_, AppState>) -> Result<launch::GameStatus> {
    Ok(launch::status(&state).await)
}

// --------------------------------------------------------------------- run

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let paths = Paths::resolve(app.handle())?;
            instance::bootstrap(&paths)?;
            app.manage(AppState {
                paths,
                http: modrinth::http_client()?,
                install_lock: tokio::sync::Mutex::new(()),
                auth: tokio::sync::Mutex::new(auth::AuthState::default()),
                game: tokio::sync::Mutex::new(None),
                launching: std::sync::atomic::AtomicBool::new(false),
                game_log: launch::new_log(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_overview,
            list_content,
            set_mod_enabled,
            set_resource_packs,
            set_active_shader,
            remove_content,
            open_content_folder,
            import_files,
            modrinth_search,
            modrinth_install,
            get_hud_config,
            save_hud_config,
            default_hud_config,
            list_profiles,
            set_active_profile,
            get_accounts,
            set_client_id,
            auth_begin,
            auth_cancel,
            auth_open_link,
            select_account,
            sign_out,
            add_test_account,
            list_versions,
            set_version,
            launch_game,
            stop_game,
            game_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Strata");
}
