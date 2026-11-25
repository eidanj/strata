//! Microsoft sign-in for Minecraft: Java Edition.
//!
//! Device code flow (the user types a short code at microsoft.com/link), then
//! Xbox Live -> XSTS -> Minecraft services -> profile. The Microsoft refresh
//! token goes to the OS credential store; the Minecraft access token only
//! ever lives in memory.
//!
//! Your Azure app must be approved for the Minecraft API
//! (https://aka.ms/mce-reviewappid) or `login_with_xbox` answers 403.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{Error, Result};
use crate::instance;
use crate::paths::{write_json_atomic, Paths};
use crate::AppState;

const MS_DEVICE_CODE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MS_TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_AUTH: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";
const KEYCHAIN_SERVICE: &str = "gg.strata.launcher";

pub const STATUS_EVENT: &str = "auth-status";

// ------------------------------------------------------------------ model

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Microsoft,
    /// Offline account for testing the launcher. Only exists in debug builds.
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub uuid: String,
    pub name: String,
    pub kind: AccountKind,
    #[serde(default)]
    pub skin_variant: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsFile {
    pub active: Option<String>,
    #[serde(default)]
    pub accounts: Vec<Account>,
}

impl AccountsFile {
    pub fn load(paths: &Paths) -> Result<Self> {
        match std::fs::read(paths.accounts_file()) {
            Ok(b) => Ok(serde_json::from_slice(&b)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_json_atomic(&paths.accounts_file(), self)
    }
    pub fn active_account(&self) -> Option<&Account> {
        let id = self.active.as_deref()?;
        self.accounts.iter().find(|a| a.uuid == id)
    }
    fn upsert(&mut self, account: Account) {
        self.accounts.retain(|a| a.uuid != account.uuid);
        self.active = Some(account.uuid.clone());
        self.accounts.push(account);
    }
}

/// In-memory auth state held in `AppState`.
#[derive(Default)]
pub struct AuthState {
    sessions: HashMap<String, Session>,
    /// Used when the OS credential store isn't available (some Linux setups).
    memory_refresh: HashMap<String, String>,
    flow_generation: u64,
    verification_uri: Option<String>,
}

#[derive(Clone)]
struct Session {
    access_token: String,
    xuid: String,
    expires_at: Instant,
}

/// What the launcher needs to start the game as this player.
pub struct Identity {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: String,
    pub user_type: &'static str,
}

// ------------------------------------------------------------ frontend view

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub uuid: String,
    pub name: String,
    pub kind: AccountKind,
    pub active: bool,
    pub skin: Option<String>,
    pub skin_variant: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsView {
    pub accounts: Vec<AccountView>,
    pub client_id_set: bool,
    pub test_accounts_allowed: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    /// "waiting" | "signing-in" | "done" | "failed" | "cancelled"
    pub state: &'static str,
    pub message: Option<String>,
}

pub fn client_id(paths: &Paths) -> Option<String> {
    instance::settings(paths)
        .ok()
        .and_then(|s| s.microsoft_client_id)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| option_env!("STRATA_MS_CLIENT_ID").map(str::to_string))
}

pub fn set_client_id(paths: &Paths, id: &str) -> Result<()> {
    let id = id.trim();
    let looks_right = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if !looks_right {
        return Err(Error::msg(
            "That doesn't look like an Application (client) ID. It's 36 characters, like 1b2c3d4e-....",
        ));
    }
    let mut settings = instance::settings(paths)?;
    settings.microsoft_client_id = Some(id.to_string());
    instance::save_settings(paths, &settings)
}

pub fn view(paths: &Paths) -> Result<AccountsView> {
    let file = AccountsFile::load(paths)?;
    let active = file.active.clone();
    let accounts = file
        .accounts
        .iter()
        .map(|a| AccountView {
            uuid: a.uuid.clone(),
            name: a.name.clone(),
            kind: a.kind,
            active: active.as_deref() == Some(&a.uuid),
            skin: skin_data_url(paths, a),
            skin_variant: a.skin_variant.clone().unwrap_or_else(|| "classic".into()).to_lowercase(),
        })
        .collect();
    Ok(AccountsView {
        accounts,
        client_id_set: client_id(paths).is_some(),
        test_accounts_allowed: cfg!(debug_assertions),
    })
}

// --------------------------------------------------------- device code flow

/// Start sign-in. Returns the code to show; polling continues in the
/// background and reports through `auth-status` events.
pub async fn begin(app: AppHandle, state: &AppState) -> Result<DeviceCode> {
    let client_id = client_id(&state.paths)
        .ok_or_else(|| Error::msg("Add your Azure app's client ID first (Home, under the sign-in button)."))?;

    let resp: Value = state
        .http
        .post(MS_DEVICE_CODE)
        .form(&[("client_id", client_id.as_str()), ("scope", SCOPE)])
        .send()
        .await?
        .json()
        .await?;
    if let Some(err) = resp.get("error_description").and_then(Value::as_str) {
        return Err(Error::msg(format!("Microsoft refused to start sign-in: {err}")));
    }
    let code = DeviceCode {
        user_code: str_field(&resp, "user_code")?,
        verification_uri: str_field(&resp, "verification_uri")?,
        expires_in: resp.get("expires_in").and_then(Value::as_u64).unwrap_or(900),
    };
    let device_code = str_field(&resp, "device_code")?;
    let interval = resp.get("interval").and_then(Value::as_u64).unwrap_or(5);

    let generation = {
        let mut auth = state.auth.lock().await;
        auth.flow_generation += 1;
        auth.verification_uri = Some(code.verification_uri.clone());
        auth.flow_generation
    };

    let expires = code.expires_in;
    tauri::async_runtime::spawn(async move {
        let result = poll(&app, &client_id, &device_code, interval, expires, generation).await;
        let status = match result {
            Ok(Some(name)) => AuthStatus { state: "done", message: Some(format!("Signed in as {name}")) },
            Ok(None) => AuthStatus { state: "cancelled", message: None },
            Err(e) => AuthStatus { state: "failed", message: Some(e.to_string()) },
        };
        let _ = app.emit(STATUS_EVENT, status);
    });
    Ok(code)
}

pub async fn cancel(state: &AppState) {
    let mut auth = state.auth.lock().await;
    auth.flow_generation += 1;
    auth.verification_uri = None;
}

pub async fn verification_uri(state: &AppState) -> Option<String> {
    state.auth.lock().await.verification_uri.clone()
}

/// `Ok(None)` if cancelled.
async fn poll(app: &AppHandle, client_id: &str, device_code: &str, mut interval: u64, expires_in: u64, generation: u64) -> Result<Option<String>> {
    let state = app.state::<AppState>();
    let deadline = Instant::now() + Duration::from_secs(expires_in);

    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if state.auth.lock().await.flow_generation != generation {
            return Ok(None);
        }
        if Instant::now() > deadline {
            return Err(Error::msg("The sign-in code expired. Start again and enter the new code."));
        }

        let resp: Value = state
            .http
            .post(MS_TOKEN)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", device_code),
            ])
            .send()
            .await?
            .json()
            .await?;

        match resp.get("error").and_then(Value::as_str) {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += 5;
                continue;
            }
            Some("authorization_declined") => return Err(Error::msg("Sign-in was declined.")),
            Some("expired_token") => return Err(Error::msg("The sign-in code expired. Start again.")),
            Some(other) => {
                let desc = resp.get("error_description").and_then(Value::as_str).unwrap_or(other);
                return Err(Error::msg(format!("Microsoft sign-in failed: {desc}")));
            }
            None => {}
        }

        let _ = app.emit(STATUS_EVENT, AuthStatus { state: "signing-in", message: Some("Connecting to Xbox Live".into()) });
        let ms_access = str_field(&resp, "access_token")?;
        let ms_refresh = str_field(&resp, "refresh_token")?;
        let account = finish(&state, &ms_access, &ms_refresh).await?;
        state.auth.lock().await.verification_uri = None;
        return Ok(Some(account.name));
    }
}

/// Microsoft token -> Minecraft session + profile; stores everything.
async fn finish(state: &AppState, ms_access: &str, ms_refresh: &str) -> Result<Account> {
    let (mc_token, expires_in, xuid) = minecraft_login(&state.http, ms_access).await?;
    let profile = fetch_profile(&state.http, &mc_token).await?;

    let uuid = dashed(&str_field(&profile, "id")?);
    let name = str_field(&profile, "name")?;
    let active_skin = profile
        .get("skins")
        .and_then(Value::as_array)
        .and_then(|skins| skins.iter().find(|s| s.get("state").and_then(Value::as_str) == Some("ACTIVE")).or(skins.first()))
        .cloned();
    let skin_variant = active_skin.as_ref().and_then(|s| s.get("variant")).and_then(Value::as_str).map(str::to_lowercase);
    if let Some(url) = active_skin.as_ref().and_then(|s| s.get("url")).and_then(Value::as_str) {
        if let Err(e) = save_skin(state, &uuid, url).await {
            eprintln!("Couldn't download skin: {e}");
        }
    }

    {
        let mut auth = state.auth.lock().await;
        if store_refresh(&uuid, ms_refresh).is_err() {
            auth.memory_refresh.insert(uuid.clone(), ms_refresh.to_string());
        }
        auth.sessions.insert(
            uuid.clone(),
            Session { access_token: mc_token, xuid, expires_at: Instant::now() + Duration::from_secs(expires_in) },
        );
    }

    let account = Account { uuid, name, kind: AccountKind::Microsoft, skin_variant };
    let mut file = AccountsFile::load(&state.paths)?;
    file.upsert(account.clone());
    file.save(&state.paths)?;
    Ok(account)
}

/// Xbox Live -> XSTS -> Minecraft. Returns (access token, seconds valid, xuid).
async fn minecraft_login(http: &reqwest::Client, ms_access: &str) -> Result<(String, u64, String)> {
    let xbl: Value = http
        .post(XBL_AUTH)
        .header("Accept", "application/json")
        .json(&json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}") },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let xbl_token = str_field(&xbl, "Token")?;

    let xsts_resp = http
        .post(XSTS_AUTH)
        .header("Accept", "application/json")
        .json(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;
    if xsts_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        let body: Value = xsts_resp.json().await.unwrap_or(Value::Null);
        return Err(Error::msg(xsts_error(body.get("XErr").and_then(Value::as_u64).unwrap_or(0))));
    }
    let xsts: Value = xsts_resp.error_for_status()?.json().await?;
    let xsts_token = str_field(&xsts, "Token")?;
    let uhs = xsts
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::msg("Xbox Live returned no user hash"))?;

    let mc_resp = http
        .post(MC_LOGIN)
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }))
        .send()
        .await?;
    if matches!(mc_resp.status().as_u16(), 401 | 403) {
        return Err(Error::msg(
            "Minecraft refused this app's sign-in. Your Azure app isn't approved for the Minecraft API yet; apply at aka.ms/mce-reviewappid.",
        ));
    }
    let mc: Value = mc_resp.error_for_status()?.json().await?;
    let token = str_field(&mc, "access_token")?;
    let expires_in = mc.get("expires_in").and_then(Value::as_u64).unwrap_or(86_400);
    let xuid = jwt_claim(&token, "xuid").unwrap_or_else(|| "0".into());
    Ok((token, expires_in, xuid))
}

async fn fetch_profile(http: &reqwest::Client, mc_token: &str) -> Result<Value> {
    let resp = http.get(MC_PROFILE).bearer_auth(mc_token).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::msg("This Microsoft account doesn't own Minecraft: Java Edition."));
    }
    Ok(resp.error_for_status()?.json().await?)
}

fn xsts_error(code: u64) -> String {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at xbox.com, then try again.".into(),
        2148916235 => "Xbox Live isn't available in this account's country or region.".into(),
        2148916236 | 2148916237 => "This account needs adult verification on xbox.com before it can sign in.".into(),
        2148916238 => "This is a child account. An adult needs to add it to a Microsoft family group first.".into(),
        other => format!("Xbox Live refused sign-in (error {other})."),
    }
}

// ----------------------------------------------------------- launch tokens

/// A valid identity for the active account, refreshing tokens if needed.
pub async fn identity(state: &AppState) -> Result<Identity> {
    let file = AccountsFile::load(&state.paths)?;
    let account = file.active_account().cloned().ok_or_else(|| Error::msg("Sign in on the Home tab first."))?;

    if account.kind == AccountKind::Test {
        if !cfg!(debug_assertions) {
            return Err(Error::msg("Test accounts only work in development builds. Sign in with Microsoft."));
        }
        return Ok(Identity { name: account.name, uuid: account.uuid, access_token: "0".into(), xuid: "0".into(), user_type: "legacy" });
    }

    let cached = state.auth.lock().await.sessions.get(&account.uuid).cloned();
    if let Some(s) = cached {
        if s.expires_at > Instant::now() + Duration::from_secs(300) {
            return Ok(Identity { name: account.name, uuid: account.uuid, access_token: s.access_token, xuid: s.xuid, user_type: "msa" });
        }
    }

    // Refresh: Microsoft refresh token -> new Microsoft access token -> Minecraft.
    let client_id = client_id(&state.paths).ok_or_else(|| Error::msg("Microsoft client ID is missing."))?;
    let refresh = {
        let auth = state.auth.lock().await;
        auth.memory_refresh.get(&account.uuid).cloned()
    }
    .or_else(|| load_refresh(&account.uuid).ok())
    .ok_or_else(|| Error::msg("Your sign-in expired. Sign in again on the Home tab."))?;

    let resp: Value = state
        .http
        .post(MS_TOKEN)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("refresh_token", refresh.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?
        .json()
        .await?;
    if resp.get("error").is_some() {
        return Err(Error::msg("Your sign-in expired. Sign in again on the Home tab."));
    }
    let ms_access = str_field(&resp, "access_token")?;
    let new_refresh = resp.get("refresh_token").and_then(Value::as_str).unwrap_or(&refresh).to_string();
    let (token, expires_in, xuid) = minecraft_login(&state.http, &ms_access).await?;

    let mut auth = state.auth.lock().await;
    if store_refresh(&account.uuid, &new_refresh).is_err() {
        auth.memory_refresh.insert(account.uuid.clone(), new_refresh);
    }
    auth.sessions.insert(
        account.uuid.clone(),
        Session { access_token: token.clone(), xuid: xuid.clone(), expires_at: Instant::now() + Duration::from_secs(expires_in) },
    );
    Ok(Identity { name: account.name, uuid: account.uuid, access_token: token, xuid, user_type: "msa" })
}

// --------------------------------------------------------- account changes

pub fn select(paths: &Paths, uuid: &str) -> Result<()> {
    let mut file = AccountsFile::load(paths)?;
    if !file.accounts.iter().any(|a| a.uuid == uuid) {
        return Err(Error::msg("That account isn't signed in anymore."));
    }
    file.active = Some(uuid.to_string());
    file.save(paths)
}

pub async fn sign_out(state: &AppState, uuid: &str) -> Result<()> {
    let mut file = AccountsFile::load(&state.paths)?;
    file.accounts.retain(|a| a.uuid != uuid);
    if file.active.as_deref() == Some(uuid) {
        file.active = file.accounts.first().map(|a| a.uuid.clone());
    }
    file.save(&state.paths)?;
    let _ = delete_refresh(uuid);
    let _ = std::fs::remove_file(state.paths.skins_dir().join(format!("{uuid}.png")));
    let mut auth = state.auth.lock().await;
    auth.sessions.remove(uuid);
    auth.memory_refresh.remove(uuid);
    Ok(())
}

/// Offline account for testing the launch pipeline before your Azure app is
/// approved. Compiled out of release builds' behaviour: `identity` refuses it.
pub fn add_test_account(paths: &Paths, name: &str) -> Result<()> {
    if !cfg!(debug_assertions) {
        return Err(Error::msg("Test accounts are only available in development builds."));
    }
    let valid = (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(Error::msg("Use 3 to 16 letters, numbers or underscores."));
    }
    let mut file = AccountsFile::load(paths)?;
    file.upsert(Account { uuid: offline_uuid(name), name: name.to_string(), kind: AccountKind::Test, skin_variant: None });
    file.save(paths)
}

// ------------------------------------------------------------------- skins

async fn save_skin(state: &AppState, uuid: &str, url: &str) -> Result<()> {
    let url = url.replacen("http://", "https://", 1);
    let bytes = state.http.get(url).send().await?.error_for_status()?.bytes().await?;
    if bytes.len() > 256 * 1024 || !bytes.starts_with(b"\x89PNG") {
        return Err(Error::msg("Skin isn't a PNG"));
    }
    let dir = state.paths.skins_dir();
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join(format!("{uuid}.png")), &bytes).await?;
    Ok(())
}

/// The account's skin as a data URL. Test accounts (and accounts whose skin
/// failed to download) fall back to the default skin read from the game jar
/// once it's been downloaded, so Strata never ships Mojang's artwork.
fn skin_data_url(paths: &Paths, account: &Account) -> Option<String> {
    let png = std::fs::read(paths.skins_dir().join(format!("{}.png", account.uuid))).ok().or_else(|| default_skin(paths))?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

fn default_skin(paths: &Paths) -> Option<Vec<u8>> {
    use std::io::Read;
    let id = instance::active_id(paths).ok()?;
    let mc = instance::load(paths, &id).ok()?.minecraft_version;
    let jar = paths.versions_dir().join(&mc).join(format!("{mc}.jar"));
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut entry = zip.by_name("assets/minecraft/textures/entity/player/wide/steve.png").ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

// ----------------------------------------------------------------- helpers

fn keychain(uuid: &str) -> std::result::Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYCHAIN_SERVICE, uuid)
}

fn store_refresh(uuid: &str, token: &str) -> std::result::Result<(), keyring::Error> {
    keychain(uuid)?.set_password(token)
}

fn load_refresh(uuid: &str) -> std::result::Result<String, keyring::Error> {
    keychain(uuid)?.get_password()
}

fn delete_refresh(uuid: &str) -> std::result::Result<(), keyring::Error> {
    keychain(uuid)?.delete_credential()
}

fn str_field(v: &Value, key: &str) -> Result<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::msg(format!("Unexpected response: missing {key}")))
}

/// Read one claim from a JWT's payload without verifying it (we only need
/// the xuid Minecraft puts there).
fn jwt_claim(token: &str, claim: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    match v.get(claim)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Mojang returns undashed UUIDs; the game wants them dashed.
fn dashed(id: &str) -> String {
    if id.len() != 32 {
        return id.to_string();
    }
    format!("{}-{}-{}-{}-{}", &id[0..8], &id[8..12], &id[12..16], &id[16..20], &id[20..32])
}

/// Same as Java's `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`, which is
/// what offline-mode servers expect.
pub fn offline_uuid(name: &str) -> String {
    use md5::{Digest, Md5};
    let mut b: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    dashed(&hex::encode(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java() {
        // Values from java.util.UUID.nameUUIDFromBytes("OfflinePlayer:<name>")
        assert_eq!(offline_uuid("Notch"), "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert_eq!(offline_uuid("Eidan_Test"), "c2339463-df3c-38da-9e55-2dad6063ab36");
    }

    #[test]
    fn reads_xuid_from_jwt() {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"xuid":"2535400000000000","sub":"x"}"#);
        let token = format!("eyJhbGciOiJIUzI1NiJ9.{payload}.sig");
        assert_eq!(jwt_claim(&token, "xuid").as_deref(), Some("2535400000000000"));
        assert_eq!(jwt_claim("not-a-jwt", "xuid"), None);
    }

    #[test]
    fn dashes_uuids() {
        assert_eq!(dashed("069a79f444e94726a5befca90e38aaf5"), "069a79f4-44e9-4726-a5be-fca90e38aaf5");
    }
}
