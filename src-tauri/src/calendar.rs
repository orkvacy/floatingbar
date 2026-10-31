//! Read-only Google Calendar integration. All network and OAuth work runs off the UI thread.
use crate::secure_store::protect;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tiny_http::{Response, Server};
use url::Url;

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const LIST_SCOPE: &str = "https://www.googleapis.com/auth/calendar.calendarlist.readonly";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events.readonly https://www.googleapis.com/auth/calendar.calendarlist.readonly";

#[derive(Default)]
pub struct CalendarManager {
    operation: Mutex<()>,
    generation: AtomicU64,
}

#[derive(Serialize, Deserialize)]
struct Credentials {
    client_id: String,
    client_secret: String,
}

#[derive(Serialize, Deserialize)]
struct Account {
    credentials: Credentials,
    refresh_token: String,
    access_token: String,
    expires_at: i64,
    #[serde(default)]
    granted_scope: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    id: String,
    title: String,
    start: String,
    end: String,
    all_day: bool,
    location: Option<String>,
    url: Option<String>,
    calendar_id: String,
    calendar_name: String,
    calendar_color: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CalendarSource {
    id: String,
    name: String,
    color: String,
    selected: bool,
    primary: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarAgenda {
    events: Vec<CalendarEvent>,
    calendars: Vec<CalendarSource>,
    warnings: Vec<String>,
    needs_reconnect: bool,
}

fn client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Tidak bisa membuat koneksi Google".into())
}

fn account_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("google-calendar.dat"))
        .map_err(|e| e.to_string())
}

fn load_account(app: &AppHandle) -> Result<Option<Account>, String> {
    let path = account_path(app)?;
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Tidak bisa membaca akun kalender".into()),
    };
    serde_json::from_slice(&protect(&bytes, false)?)
        .map(Some)
        .map_err(|_| "Data akun kalender rusak. Putuskan koneksi lalu hubungkan ulang.".into())
}

fn save_account(app: &AppHandle, account: &Account) -> Result<(), String> {
    crate::secure_store::write(&account_path(app)?, account)
}

fn parse_credentials(json: &str) -> Result<Credentials, String> {
    if json.len() > 64 * 1024 {
        return Err("File kredensial terlalu besar".into());
    }
    let doc: Value =
        serde_json::from_str(json).map_err(|_| "File bukan JSON kredensial Google yang valid")?;
    let credentials: Credentials = serde_json::from_value(
        doc.get("installed")
            .cloned()
            .ok_or("Pilih kredensial OAuth bertipe Desktop app, bukan Web application")?,
    )
    .map_err(|_| "Client ID atau client secret tidak ditemukan")?;
    if !credentials
        .client_id
        .ends_with(".apps.googleusercontent.com")
        || credentials.client_secret.is_empty()
    {
        return Err("Kredensial Google tidak valid".into());
    }
    Ok(credentials)
}

fn random_key() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "Tidak bisa membuat sesi login aman")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn token_request(client: &Client, params: &[(&str, &str)]) -> Result<Value, String> {
    let response = client
        .post(TOKEN_URL)
        .form(params)
        .send()
        .map_err(|_| "Tidak bisa menghubungi Google. Periksa koneksi internet.")?;
    let status = response.status();
    let data: Value = response
        .json()
        .map_err(|_| "Respons token Google tidak valid")?;
    if !status.is_success() {
        return Err(match data["error"].as_str() {
            Some("invalid_grant") => {
                "Izin Google sudah berakhir atau dicabut. Hubungkan ulang kalender."
            }
            Some("invalid_client") => {
                "Kredensial OAuth tidak cocok. Pilih JSON Desktop app yang benar."
            }
            _ => "Login Google gagal. Periksa pengaturan OAuth lalu coba lagi.",
        }
        .into());
    }
    Ok(data)
}

fn callback_code(target: &str, expected_state: &str) -> Result<Option<String>, String> {
    let url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "Callback login tidak valid")?;
    if url.path() != "/oauth/callback" {
        return Ok(None);
    }
    let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Ok(None);
    }
    if params.contains_key("error") {
        return Err("Izin kalender tidak diberikan. Kamu bisa mencoba lagi.".into());
    }
    params
        .get("code")
        .cloned()
        .map(Some)
        .ok_or("Kode login Google tidak ditemukan".into())
}

fn connect(app: &AppHandle, json: Option<&str>) -> Result<(), String> {
    let manager = app.state::<CalendarManager>();
    let _guard = manager
        .operation
        .try_lock()
        .map_err(|_| "Kalender masih memproses permintaan lain. Coba lagi sebentar.")?;
    let generation = manager.generation.load(Ordering::SeqCst);
    let credentials = match json {
        Some(json) => parse_credentials(json)?,
        None => {
            load_account(app)?
                .ok_or("Hubungkan Google Calendar terlebih dahulu")?
                .credentials
        }
    };
    let verifier = random_key()?;
    let state = random_key()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let server =
        Server::http("127.0.0.1:0").map_err(|_| "Tidak bisa membuka callback login lokal")?;
    let address = server
        .server_addr()
        .to_ip()
        .ok_or("Alamat callback tidak tersedia")?;
    let redirect = format!("http://127.0.0.1:{}/oauth/callback", address.port());
    let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", credentials.client_id.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("access_type", "offline"),
        ("prompt", "consent select_account"),
        ("state", state.as_str()),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
    ]);
    crate::shortcuts::launch(url.as_str())?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let code = loop {
        if manager.generation.load(Ordering::SeqCst) != generation {
            return Err("Login dibatalkan".into());
        }
        if Instant::now() >= deadline {
            return Err("Waktu login habis. Silakan hubungkan ulang.".into());
        }
        let Some(request) = server
            .recv_timeout(Duration::from_millis(250))
            .map_err(|_| "Callback Google gagal")?
        else {
            continue;
        };
        if request.method() != &tiny_http::Method::Get {
            let _ =
                request.respond(Response::from_string("Method not allowed").with_status_code(405));
            continue;
        }
        match callback_code(request.url(), &state) {
            Ok(Some(code)) => {
                let _ = request.respond(Response::from_string(
                    "Login Google diterima. Silakan kembali ke FloatingBar.",
                ));
                break code;
            }
            Ok(None) => {
                let _ = request.respond(Response::from_string("Not found").with_status_code(404));
            }
            Err(error) => {
                let _ = request.respond(Response::from_string(
                    "Izin tidak diberikan. Kembali ke FloatingBar.",
                ));
                return Err(error);
            }
        }
    };
    let data = token_request(
        &client()?,
        &[
            ("client_id", &credentials.client_id),
            ("client_secret", &credentials.client_secret),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect),
            ("grant_type", "authorization_code"),
        ],
    )?;
    if manager.generation.load(Ordering::SeqCst) != generation {
        return Err("Login dibatalkan".into());
    }
    let access_token = data["access_token"]
        .as_str()
        .ok_or("Token Google tidak ditemukan")?
        .to_owned();
    let refresh_token = data["refresh_token"].as_str().ok_or("Google tidak memberi akses offline. Cabut izin FloatingBar di akun Google lalu coba lagi.")?.to_owned();
    save_account(
        app,
        &Account {
            credentials,
            access_token,
            refresh_token,
            expires_at: Utc::now().timestamp() + data["expires_in"].as_i64().unwrap_or(3600),
            granted_scope: data["scope"].as_str().unwrap_or(SCOPE).into(),
        },
    )
}

fn day_range(start: &str, end: &str) -> Result<(), String> {
    let start = DateTime::parse_from_rfc3339(start).map_err(|_| "Awal hari tidak valid")?;
    let end = DateTime::parse_from_rfc3339(end).map_err(|_| "Akhir hari tidak valid")?;
    let hours = (end - start).num_hours();
    if !(20..=28).contains(&hours) {
        return Err("Rentang agenda harus satu hari".into());
    }
    Ok(())
}

fn parse_events(items: &[Value], calendar: &CalendarSource) -> Vec<CalendarEvent> {
    items
        .iter()
        .filter(|v| v["status"] != "cancelled")
        .filter_map(|v| {
            let all_day = v["start"]["date"].is_string();
            Some(CalendarEvent {
                id: format!("{}:{}", calendar.id, v["id"].as_str()?),
                title: v["summary"].as_str().unwrap_or("Tanpa judul").into(),
                start: v["start"]["dateTime"]
                    .as_str()
                    .or_else(|| v["start"]["date"].as_str())?
                    .into(),
                end: v["end"]["dateTime"]
                    .as_str()
                    .or_else(|| v["end"]["date"].as_str())?
                    .into(),
                all_day,
                location: v["location"].as_str().map(str::to_owned),
                url: v["htmlLink"].as_str().map(str::to_owned),
                calendar_id: calendar.id.clone(),
                calendar_name: calendar.name.clone(),
                calendar_color: calendar.color.clone(),
            })
        })
        .collect()
}

fn primary_calendar() -> CalendarSource {
    CalendarSource {
        id: "primary".into(),
        name: "Kalender utama".into(),
        color: "#81c995".into(),
        selected: true,
        primary: true,
    }
}

fn parse_calendars(items: &[Value]) -> Vec<CalendarSource> {
    items
        .iter()
        .filter(|v| v["deleted"] != true && v["accessRole"] != "freeBusyReader")
        .filter_map(|v| {
            let primary = v["primary"].as_bool().unwrap_or(false);
            Some(CalendarSource {
                id: v["id"].as_str()?.into(),
                name: v["summaryOverride"]
                    .as_str()
                    .or_else(|| v["summary"].as_str())
                    .unwrap_or("Kalender")
                    .into(),
                color: v["backgroundColor"].as_str().unwrap_or("#81c995").into(),
                selected: v["selected"].as_bool().unwrap_or(primary),
                primary,
            })
        })
        .collect()
}

fn selected_calendars(
    calendars: Vec<CalendarSource>,
    ids: Option<&[String]>,
) -> Vec<CalendarSource> {
    calendars
        .into_iter()
        .map(|mut calendar| {
            if let Some(ids) = ids {
                calendar.selected = ids.contains(&calendar.id);
            }
            calendar
        })
        .collect()
}

fn sort_events(events: &mut [CalendarEvent]) {
    events.sort_by(|a, b| {
        b.all_day
            .cmp(&a.all_day)
            .then_with(|| {
                if a.all_day {
                    a.start.cmp(&b.start)
                } else {
                    let timestamp = |s: &str| {
                        DateTime::parse_from_rfc3339(s)
                            .map(|d| d.timestamp_millis())
                            .unwrap_or(i64::MAX)
                    };
                    timestamp(&a.start).cmp(&timestamp(&b.start))
                }
            })
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn fetch_pages(
    client: &Client,
    token: &str,
    url: Url,
    params: &[(&str, &str)],
    deadline: Instant,
) -> Result<Vec<Value>, String> {
    let mut items = Vec::new();
    let mut page = String::new();
    for _ in 0..10 {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("Waktu sinkronisasi habis. Coba perbarui lagi.".into());
        }
        let response = client
            .get(url.clone())
            .bearer_auth(token)
            .query(params)
            .query(&[("pageToken", &page)])
            .timeout(remaining.min(Duration::from_secs(20)))
            .send()
            .map_err(|_| "Koneksi ke Google gagal")?;
        match response.status().as_u16() {
            200 => {}
            401 => return Err("Izin Google berakhir. Hubungkan ulang kalender.".into()),
            403 => {
                return Err(
                    "Akses ditolak Google. Periksa izin baca kalender dan Google Calendar API."
                        .into(),
                )
            }
            404 => return Err("Kalender sudah dihapus atau tidak lagi dibagikan".into()),
            429 => return Err("Terlalu banyak permintaan. Coba lagi nanti.".into()),
            _ => return Err("Google Calendar sedang tidak tersedia".into()),
        }
        let data: Value = response.json().map_err(|_| "Respons Google tidak valid")?;
        items.extend(
            data["items"]
                .as_array()
                .ok_or("Daftar Google tidak valid")?
                .iter()
                .cloned(),
        );
        match data["nextPageToken"].as_str() {
            Some(next) => page = next.into(),
            None => return Ok(items),
        }
    }
    Err("Terlalu banyak data kalender untuk ditampilkan".into())
}

fn events_url(calendar_id: &str) -> Url {
    let mut url = Url::parse("https://www.googleapis.com/calendar/v3/calendars/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .push(calendar_id)
        .push("events");
    url
}

fn events(
    app: &AppHandle,
    start: &str,
    end: &str,
    ids: Option<&[String]>,
) -> Result<CalendarAgenda, String> {
    day_range(start, end)?;
    if ids.is_some_and(|ids| ids.len() > 2500) {
        return Err("Terlalu banyak kalender dipilih".into());
    }
    let manager = app.state::<CalendarManager>();
    let _guard = manager
        .operation
        .try_lock()
        .map_err(|_| "Kalender masih memproses permintaan lain")?;
    let mut account = load_account(app)?.ok_or("Hubungkan Google Calendar terlebih dahulu")?;
    let client = client()?;
    if account.expires_at <= Utc::now().timestamp() + 60 {
        let data = token_request(
            &client,
            &[
                ("client_id", &account.credentials.client_id),
                ("client_secret", &account.credentials.client_secret),
                ("refresh_token", &account.refresh_token),
                ("grant_type", "refresh_token"),
            ],
        )?;
        account.access_token = data["access_token"]
            .as_str()
            .ok_or("Token Google tidak ditemukan")?
            .into();
        account.expires_at = Utc::now().timestamp() + data["expires_in"].as_i64().unwrap_or(3600);
        if let Some(token) = data["refresh_token"].as_str() {
            account.refresh_token = token.into();
        }
        if let Some(scope) = data["scope"].as_str() {
            account.granted_scope = scope.into();
        }
        save_account(app, &account)?;
    }

    let deadline = Instant::now() + Duration::from_secs(45);
    let needs_reconnect = !account.granted_scope.split_whitespace().any(|scope| {
        scope == LIST_SCOPE || scope == "https://www.googleapis.com/auth/calendar.readonly"
    });
    let calendars = if needs_reconnect {
        vec![primary_calendar()]
    } else {
        parse_calendars(&fetch_pages(
            &client,
            &account.access_token,
            Url::parse("https://www.googleapis.com/calendar/v3/users/me/calendarList").unwrap(),
            &[
                ("maxResults", "250"),
                ("showHidden", "true"),
                ("minAccessRole", "reader"),
            ],
            deadline,
        )?)
    };
    // Old tokens retain primary-calendar access until the user grants list access.
    let calendars = selected_calendars(calendars, if needs_reconnect { None } else { ids });
    let selected: Vec<_> = calendars.iter().filter(|c| c.selected).collect();
    let mut all_events = Vec::new();
    let mut warnings = Vec::new();
    let mut successes = 0;
    // Bound concurrency; one unavailable shared calendar must not hide all other agendas.
    for batch in selected.chunks(4) {
        std::thread::scope(|scope| {
            let jobs: Vec<_> = batch
                .iter()
                .map(|calendar| {
                    let client = &client;
                    let token = &account.access_token;
                    (
                        *calendar,
                        scope.spawn(move || {
                            fetch_pages(
                                client,
                                token,
                                events_url(&calendar.id),
                                &[
                                    ("timeMin", start),
                                    ("timeMax", end),
                                    ("singleEvents", "true"),
                                    ("orderBy", "startTime"),
                                    ("maxResults", "250"),
                                ],
                                deadline,
                            )
                        }),
                    )
                })
                .collect();
            for (calendar, job) in jobs {
                match job
                    .join()
                    .unwrap_or_else(|_| Err("Sinkronisasi gagal".into()))
                {
                    Ok(items) => {
                        successes += 1;
                        all_events.extend(parse_events(&items, calendar));
                    }
                    Err(error) => warnings.push(format!("{}: {error}", calendar.name)),
                }
            }
        });
    }
    if !selected.is_empty() && successes == 0 {
        return Err(warnings.join("; "));
    }
    sort_events(&mut all_events);
    Ok(CalendarAgenda {
        events: all_events,
        calendars,
        warnings,
        needs_reconnect,
    })
}

#[tauri::command]
pub async fn calendar_status(app: AppHandle) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || load_account(&app).map(|a| a.is_some()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn calendar_connect(
    app: AppHandle,
    credentials_json: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || connect(&app, credentials_json.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn calendar_cancel(app: AppHandle) {
    app.state::<CalendarManager>()
        .generation
        .fetch_add(1, Ordering::SeqCst);
}

#[tauri::command]
pub async fn calendar_events(
    app: AppHandle,
    start: String,
    end: String,
    calendar_ids: Option<Vec<String>>,
) -> Result<CalendarAgenda, String> {
    tauri::async_runtime::spawn_blocking(move || {
        events(&app, &start, &end, calendar_ids.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn calendar_disconnect(app: AppHandle) -> Result<(), String> {
    calendar_cancel(app.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let manager = app.state::<CalendarManager>();
        let _guard = manager.operation.lock().map_err(|e| e.to_string())?;
        // Local sign-out only. Account-wide revocation remains in Google's account settings.
        match fs::remove_file(account_path(&app)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("Gagal menghapus koneksi kalender".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn calendar_open(url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|_| "Tautan kalender tidak valid")?;
    let google_event =
        parsed.host_str() == Some("www.google.com") && parsed.path().starts_with("/calendar/");
    if parsed.scheme() != "https"
        || !(google_event
            || matches!(
                parsed.host_str(),
                Some(
                    "calendar.google.com"
                        | "console.cloud.google.com"
                        | "developers.google.com"
                        | "myaccount.google.com"
                )
            ))
    {
        return Err("Tautan bukan situs Google Calendar yang didukung".into());
    }
    crate::shortcuts::launch(&url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn oauth_callback_requires_matching_state_and_path() {
        assert_eq!(
            callback_code("/oauth/callback?state=valid&code=a%2Bb", "valid").unwrap(),
            Some("a+b".into())
        );
        assert!(
            callback_code("/oauth/callback?state=wrong&code=stolen", "valid")
                .unwrap()
                .is_none()
        );
        assert!(callback_code("/other?state=valid&code=x", "valid")
            .unwrap()
            .is_none());
        assert!(callback_code("/oauth/callback?state=valid&error=access_denied", "valid").is_err());
    }

    #[test]
    fn credentials_must_be_desktop_and_do_not_supply_endpoints() {
        assert!(parse_credentials(
            r#"{"web":{"client_id":"x.apps.googleusercontent.com","client_secret":"s"}}"#
        )
        .is_err());
        assert!(parse_credentials(r#"{"installed":{"client_id":"x.apps.googleusercontent.com","client_secret":"s","token_uri":"https://untrusted.invalid"}}"#).is_ok());
    }

    #[test]
    fn events_keep_all_day_dates_and_omit_cancelled_items() {
        let events = parse_events(
            &[
                json!({"id":"all","start":{"date":"2026-10-31"},"end":{"date":"2026-11-01"}}),
                json!({"id":"cancelled","status":"cancelled"}),
                json!({"id":"time","summary":"APD","start":{"dateTime":"2026-10-31T09:00:00+08:00"},"end":{"dateTime":"2026-10-31T10:00:00+08:00"}}),
            ],
            &primary_calendar(),
        );
        assert_eq!(events.len(), 2);
        assert!(events[0].all_day);
        assert_eq!(events[0].start, "2026-10-31");
        assert!(!events[1].all_day);
    }

    #[test]
    fn day_bounds_accept_dst_but_reject_unbounded_ranges() {
        assert!(day_range("2026-10-30T16:00:00Z", "2026-10-31T16:00:00Z").is_ok());
        assert!(day_range("2026-03-08T05:00:00Z", "2026-03-09T04:00:00Z").is_ok());
        assert!(day_range("2026-10-30T16:00:00Z", "2026-11-02T16:00:00Z").is_err());
    }

    #[test]
    fn windows_vault_round_trip_does_not_store_plaintext() {
        let secret = b"private calendar refresh token";
        let encrypted = protect(secret, true).unwrap();
        assert!(!encrypted.windows(secret.len()).any(|part| part == secret));
        assert_eq!(protect(&encrypted, false).unwrap(), secret);
    }

    #[test]
    fn shared_calendars_follow_google_selection_and_skip_free_busy_access() {
        let calendars = parse_calendars(&[
            json!({"id":"personal","summary":"Pribadi","primary":true,"selected":true,"accessRole":"owner"}),
            json!({"id":"shared@example.com","summary":"Kelas","summaryOverride":"Kelas reg","selected":true,"accessRole":"reader"}),
            json!({"id":"hidden","summary":"Tidak dipilih","selected":false,"accessRole":"reader"}),
            json!({"id":"busy","accessRole":"freeBusyReader"}),
            json!({"id":"deleted","deleted":true}),
        ]);
        assert_eq!(calendars.len(), 3);
        assert_eq!(calendars[1].name, "Kelas reg");
        assert!(calendars[1].selected);
        assert!(!calendars[2].selected);
        assert_eq!(
            selected_calendars(calendars.clone(), Some(&[]))
                .iter()
                .filter(|c| c.selected)
                .count(),
            0
        );
        let selected = selected_calendars(calendars, Some(&["shared@example.com".into()]));
        assert!(!selected[0].selected);
        assert!(selected[1].selected);
    }

    #[test]
    fn events_from_different_calendars_do_not_share_react_keys() {
        let items =
            [json!({"id":"same-id","start":{"date":"2026-10-31"},"end":{"date":"2026-11-01"}})];
        let first = parse_events(&items, &primary_calendar());
        let other = CalendarSource {
            id: "shared@example.com".into(),
            ..primary_calendar()
        };
        let second = parse_events(&items, &other);
        assert_ne!(first[0].id, second[0].id);
        assert_eq!(second[0].calendar_id, "shared@example.com");
    }

    #[test]
    fn merged_agendas_sort_by_actual_time_across_timezones() {
        let mut events = parse_events(
            &[
                json!({"id":"later","start":{"dateTime":"2026-10-31T08:30:00+07:00"},"end":{"dateTime":"2026-10-31T09:00:00+07:00"}}),
                json!({"id":"earlier","start":{"dateTime":"2026-10-31T09:00:00+08:00"},"end":{"dateTime":"2026-10-31T10:00:00+08:00"}}),
                json!({"id":"all-day","start":{"date":"2026-10-31"},"end":{"date":"2026-11-01"}}),
            ],
            &primary_calendar(),
        );
        sort_events(&mut events);
        assert!(events[0].all_day);
        assert!(events[1].id.ends_with(":earlier"));
        assert!(events[2].id.ends_with(":later"));
    }

    #[test]
    fn shared_calendar_ids_are_encoded_as_one_path_segment() {
        let url = events_url("calendar/with+special@example.com");
        assert!(url.path().contains("calendar%2Fwith+special@example.com"));
        assert_eq!(url.path_segments().unwrap().count(), 5);
    }

    #[test]
    fn existing_primary_only_accounts_can_still_be_loaded_for_scope_upgrade() {
        let account: Account = serde_json::from_value(json!({
            "credentials":{"client_id":"example.apps.googleusercontent.com","client_secret":"example"},
            "refresh_token":"example", "access_token":"example", "expires_at":0
        })).unwrap();
        assert!(account.granted_scope.is_empty());
    }
}
