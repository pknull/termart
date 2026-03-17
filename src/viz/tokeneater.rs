//! Claude Tokens widget - Claude AI usage monitor
//!
//! Displays real-time Claude API usage statistics in the terminal.
//! Supports OAuth authentication with proper scopes for usage API access.

use crate::colors::{scheme_color, ColorState};
use crate::monitor::layout::{cpu_gradient_color_scheme, muted_color_scheme};
use crate::terminal::Terminal;
use crate::viz::VizState;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Write as IoWrite};
use std::net::TcpListener;
use std::time::{Duration, Instant};

const HELP_TEXT: &str = "\
CLAUDE TOKENS
────────────────────
r  Refresh now
";

// OAuth configuration
const OAUTH_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const OAUTH_SCOPES: &str = "user:inference user:profile";
const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e"; // Claude Code's client ID

/// OAuth credentials from ~/.claude/.credentials.json
#[derive(Deserialize)]
struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuthData>,
}

#[derive(Deserialize)]
struct OAuthData {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
}

/// Token stored in ~/.config/termart/claude-token.json
#[derive(Serialize, Deserialize)]
struct StoredToken {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
}

/// Usage bucket from API response
#[derive(Deserialize, Clone, Default)]
struct UsageBucket {
    utilization: f64,
    #[serde(rename = "resets_at")]
    resets_at: Option<String>,
}

/// Full usage response
#[derive(Deserialize, Clone, Default)]
struct UsageResponse {
    #[serde(rename = "five_hour")]
    five_hour: Option<UsageBucket>,
    #[serde(rename = "seven_day")]
    seven_day: Option<UsageBucket>,
    #[serde(rename = "seven_day_sonnet")]
    seven_day_sonnet: Option<UsageBucket>,
    #[serde(rename = "seven_day_opus")]
    seven_day_opus: Option<UsageBucket>,
}

/// API error response
#[derive(Deserialize)]
struct ApiError {
    #[serde(rename = "type")]
    error_type: Option<String>,
    error: Option<ApiErrorDetail>,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: Option<String>,
}

/// OAuth token response
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

pub struct TokenEaterConfig {
    pub time_step: f32,
    pub refresh_interval: u64,
    pub auth_mode: bool,
}

impl Default for TokenEaterConfig {
    fn default() -> Self {
        Self {
            time_step: 0.1,
            refresh_interval: 300,
            auth_mode: false,
        }
    }
}

/// Get config directory path
fn config_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("termart"))
}

/// Get token file path
fn token_path() -> Option<std::path::PathBuf> {
    config_dir().map(|p| p.join("claude-token.json"))
}

/// Read full stored token from config
fn read_stored_token_full() -> Option<StoredToken> {
    let path = token_path()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Check if token is expired (with 10 minute buffer)
fn is_token_expired(stored: &StoredToken) -> bool {
    if let Some(expires_at) = stored.expires_at {
        let now = chrono::Utc::now().timestamp();
        now >= (expires_at - 600) // 10 minute buffer
    } else {
        false // No expiry info, assume valid
    }
}

/// Refresh the access token using refresh_token
fn refresh_access_token(refresh_token: &str) -> Result<TokenResponse, String> {
    let token_body = serde_json::json!({
        "grant_type": "refresh_token",
        "client_id": OAUTH_CLIENT_ID,
        "refresh_token": refresh_token
    });

    let response = ureq::post(OAUTH_TOKEN_URL)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .send_json(&token_body);

    match response {
        Ok(resp) => resp
            .into_json::<TokenResponse>()
            .map_err(|e| format!("Failed to parse token: {}", e)),
        Err(ureq::Error::Status(status, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Err(format!("Refresh failed (HTTP {}): {}", status, body))
        }
        Err(e) => Err(format!("Refresh request failed: {}", e)),
    }
}

/// Save token to config
fn save_token(token: &TokenResponse) -> io::Result<()> {
    let dir =
        config_dir().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No config dir"))?;
    std::fs::create_dir_all(&dir)?;

    let path =
        token_path().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No token path"))?;

    let expires_at = token.expires_in.map(|e| chrono::Utc::now().timestamp() + e);

    let stored = StoredToken {
        access_token: token.access_token.clone(),
        refresh_token: token.refresh_token.clone(),
        expires_at,
    };

    let json = serde_json::to_string_pretty(&stored)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Read OAuth token - auto-refresh if expired
fn read_token() -> Option<String> {
    // Try our stored token first
    if let Some(stored) = read_stored_token_full() {
        // Check if expired and we have a refresh token
        if is_token_expired(&stored) {
            if let Some(ref refresh_token) = stored.refresh_token {
                // Attempt refresh
                if let Ok(new_token) = refresh_access_token(refresh_token) {
                    let _ = save_token(&new_token);
                    return Some(new_token.access_token);
                }
            }
            // Refresh failed or no refresh token - token is expired
            return None;
        }
        return Some(stored.access_token);
    }

    // Fall back to Claude credentials
    let home = std::env::var("HOME").ok()?;
    let path = format!("{}/.claude/.credentials.json", home);
    let data = std::fs::read_to_string(path).ok()?;
    let creds: Credentials = serde_json::from_str(&data).ok()?;
    creds.claude_ai_oauth?.access_token
}

/// Generate PKCE code verifier (random 64 chars)
fn generate_code_verifier() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::thread_rng();
    (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate PKCE code challenge from verifier
fn generate_code_challenge(verifier: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    base64_url_encode(&hash)
}

/// Base64 URL encode (no padding)
fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Run the OAuth authorization flow
pub fn run_auth() -> io::Result<()> {
    println!("Starting Claude OAuth authorization...");
    println!();

    // Find an available port
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://localhost:{}/callback", port);

    // Generate PKCE codes
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);

    // Generate state for CSRF protection
    let state: String = (0..32)
        .map(|_| rand::random::<u8>())
        .map(|b| format!("{:02x}", b))
        .collect();

    // Build authorization URL
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        OAUTH_AUTHORIZE_URL,
        OAUTH_CLIENT_ID,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(OAUTH_SCOPES),
        state,
        code_challenge
    );

    println!("Opening browser for authorization...");
    println!();
    println!("If browser doesn't open, visit:");
    println!("{}", auth_url);
    println!();

    // Open browser
    let _ = std::process::Command::new("xdg-open")
        .arg(&auth_url)
        .spawn();

    println!("Waiting for authorization callback...");

    // Wait for callback (with timeout)
    listener.set_nonblocking(false)?;

    let (mut stream, _) = listener.accept()?;

    // Read HTTP request
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the callback URL for the code
    let code = if let Some(query_start) = request_line.find('?') {
        let query_end = request_line.find(" HTTP").unwrap_or(request_line.len());
        let query = &request_line[query_start + 1..query_end];

        let mut code_value = None;
        let mut state_value = None;

        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                match key {
                    "code" => code_value = Some(value.to_string()),
                    "state" => state_value = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        // Verify state
        if state_value.as_deref() != Some(&state) {
            // Send error response
            let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Error: State mismatch</h1></body></html>";
            stream.write_all(response.as_bytes())?;
            return Err(io::Error::new(io::ErrorKind::InvalidData, "State mismatch"));
        }

        code_value
    } else {
        None
    };

    let code = match code {
        Some(c) => c,
        None => {
            let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Error: No authorization code</h1></body></html>";
            stream.write_all(response.as_bytes())?;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "No authorization code",
            ));
        }
    };

    // Send success response to browser
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Authorization successful!</h1><p>You can close this window and return to the terminal.</p></body></html>";
    stream.write_all(response.as_bytes())?;
    drop(stream);

    println!("Received authorization code. Exchanging for token...");

    // Exchange code for token (JSON body with browser-like headers)
    let token_body = serde_json::json!({
        "grant_type": "authorization_code",
        "client_id": OAUTH_CLIENT_ID,
        "code": code,
        "redirect_uri": redirect_uri,
        "code_verifier": code_verifier,
        "state": state
    });

    let token_response = ureq::post(OAUTH_TOKEN_URL)
        .set("Content-Type", "application/json")
        .set(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        )
        .set("Accept", "application/json")
        .set("Referer", "https://claude.ai/")
        .set("Origin", "https://claude.ai")
        .send_json(&token_body);

    match token_response {
        Ok(resp) => {
            let token: TokenResponse = resp.into_json().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse token: {}", e),
                )
            })?;

            save_token(&token)?;

            println!();
            println!("Authorization successful!");
            println!("Token saved to {:?}", token_path().unwrap_or_default());
            println!();
            println!("You can now run: termart claude-tokens");
        }
        Err(ureq::Error::Status(status, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            eprintln!("Token exchange failed (HTTP {}): {}", status, body);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("HTTP {}: {}", status, body),
            ));
        }
        Err(e) => {
            eprintln!("Failed to exchange code for token: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    }

    Ok(())
}

/// Fetch usage data from Anthropic API
fn fetch_usage(token: &str) -> Result<UsageResponse, String> {
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-H",
            &format!("Authorization: Bearer {}", token),
            "-H",
            "anthropic-beta: oauth-2025-04-20",
            "https://api.anthropic.com/api/oauth/usage",
        ])
        .output()
        .map_err(|e| format!("Failed to run curl: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "HTTP error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Check for API error response
    if let Ok(api_error) = serde_json::from_slice::<ApiError>(&output.stdout) {
        if api_error.error_type.as_deref() == Some("error") {
            if let Some(detail) = api_error.error {
                let msg = detail
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string());
                if msg.contains("scope") {
                    return Err(
                        "OAuth scope error - run 'termart claude-tokens --auth' to authorize"
                            .to_string(),
                    );
                }
                return Err(msg);
            }
        }
    }

    serde_json::from_slice(&output.stdout).map_err(|e| format!("JSON parse error: {}", e))
}

/// Parse ISO 8601 timestamp to Duration until reset
fn time_until_reset(resets_at: &str) -> Option<Duration> {
    use chrono::{DateTime, Utc};
    let reset_time: DateTime<Utc> = resets_at.parse().ok()?;
    let now = Utc::now();
    if reset_time > now {
        Some((reset_time - now).to_std().ok()?)
    } else {
        None
    }
}

/// Calculate elapsed percentage of a window given remaining time
/// window_hours: total window size (5 for 5-hour, 168 for 7-day)
fn elapsed_pct(remaining: Duration, window_hours: f64) -> f64 {
    let remaining_hours = remaining.as_secs_f64() / 3600.0;
    let elapsed_hours = (window_hours - remaining_hours).max(0.0);
    (elapsed_hours / window_hours * 100.0).min(100.0)
}

/// Format duration as human-readable
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

/// Draw a usage bar with optional pacing ghost
/// expected_pct: if Some, draws a dim underlay showing elapsed time as expected usage
fn draw_usage_bar(
    term: &mut Terminal,
    x: usize,
    y: usize,
    width: usize,
    pct: f64,
    expected_pct: Option<f64>,
    label: &str,
    colors: &ColorState,
) {
    // Layout: Label(8) + Meter(dynamic) + Pct(6)
    let label_w = 8;
    let pct_w = 6;
    let meter_w = width.saturating_sub(label_w + pct_w);

    let mut pos = x as i32;

    // Label
    let label_str = format!("{:<8}", label);
    term.set_str(
        pos,
        y as i32,
        &label_str,
        Some(muted_color_scheme(colors)),
        false,
    );
    pos += label_w as i32;

    // Draw meter with pacing ghost
    draw_meter_with_pacing(
        term,
        pos,
        y as i32,
        meter_w,
        pct as f32,
        expected_pct.map(|p| p as f32),
        colors,
    );
    pos += meter_w as i32;

    // Percentage
    let pct_str = format!("{:5.1}%", pct);
    let color = cpu_gradient_color_scheme(pct as f32, colors);
    term.set_str(pos, y as i32, &pct_str, Some(color), pct >= 80.0);
}

/// Draw meter with optional pacing ghost underlay
fn draw_meter_with_pacing(
    term: &mut Terminal,
    x: i32,
    y: i32,
    width: usize,
    percent: f32,
    expected_pct: Option<f32>,
    colors: &ColorState,
) {
    if width == 0 {
        return;
    }

    const METER_CHAR: char = '■';
    let filled = ((percent / 100.0) * width as f32) as usize;
    let expected_filled = expected_pct
        .map(|e| ((e / 100.0) * width as f32) as usize)
        .unwrap_or(0);

    // Ghost color - very dim, shows "expected" usage based on elapsed time
    let ghost_color = Color::AnsiValue(238); // Dark gray

    for i in 0..width {
        if i < filled {
            // Actual usage - gradient color
            let pos_pct = (i as f32 / width as f32) * 100.0;
            let grad = cpu_gradient_color_scheme(pos_pct.min(percent), colors);
            term.set(x + i as i32, y, METER_CHAR, Some(grad), false);
        } else if i < expected_filled {
            // Pacing ghost - dim color showing unused capacity
            term.set(x + i as i32, y, METER_CHAR, Some(ghost_color), false);
        } else {
            // Empty - muted
            term.set(
                x + i as i32,
                y,
                METER_CHAR,
                Some(muted_color_scheme(colors)),
                false,
            );
        }
    }
}

/// Draw pacing indicator inline (right-aligned to right_edge)
fn draw_pacing_inline(
    term: &mut Terminal,
    right_edge: usize,
    y: usize,
    pct: f64,
    hours_elapsed: f64,
    scheme: u8,
) {
    let expected = (hours_elapsed / 5.0) * 100.0;
    let diff = pct - expected;

    let (indicator, color, label) = if diff < -15.0 {
        ("▼", scheme_color(scheme, 0, false).0, "chill")
    } else if diff < 15.0 {
        ("●", scheme_color(scheme, 2, false).0, "on pace")
    } else {
        ("▲", scheme_color(scheme, 3, true).0, "hot")
    };

    let text = format!("{} {}", indicator, label);
    let x = right_edge.saturating_sub(text.len());
    term.set_str(x as i32, y as i32, &text, Some(color), false);
}

pub fn run(config: TokenEaterConfig) -> io::Result<()> {
    // Handle auth mode
    if config.auth_mode {
        return run_auth();
    }

    // Read token (mutable - re-read on refresh to handle token expiry)
    let mut token = match read_token() {
        Some(t) => t,
        None => {
            eprintln!("Error: No Claude token found.");
            eprintln!();
            eprintln!("Run 'termart claude-tokens --auth' to authorize with Claude.");
            return Ok(());
        }
    };

    let mut term = Terminal::new(true)?;
    let mut state = VizState::new(config.time_step, HELP_TEXT);

    let mut last_fetch = Instant::now();
    let (mut usage, mut fetch_error) = match fetch_usage(&token) {
        Ok(u) => (u, None),
        Err(e) => (UsageResponse::default(), Some(e)),
    };

    let (mut w, mut h) = term.size();

    loop {
        if let Ok(Some((code, mods))) = term.check_key() {
            if state.handle_key(code, mods) {
                break;
            }
            if code == KeyCode::Char('r') || code == KeyCode::Char('R') {
                // Re-read token (handles auto-refresh if expired)
                if let Some(fresh_token) = read_token() {
                    token = fresh_token;
                }
                match fetch_usage(&token) {
                    Ok(u) => {
                        usage = u;
                        fetch_error = None;
                    }
                    Err(e) => fetch_error = Some(e),
                }
                last_fetch = Instant::now();
            }
        }

        if let Ok((new_w, new_h)) = size() {
            if new_w != w || new_h != h {
                w = new_w;
                h = new_h;
                term.resize(w, h);
                term.clear_screen()?;
            }
        }

        if last_fetch.elapsed() > Duration::from_secs(config.refresh_interval) {
            // Re-read token (handles auto-refresh if expired)
            if let Some(fresh_token) = read_token() {
                token = fresh_token;
            }
            match fetch_usage(&token) {
                Ok(u) => {
                    usage = u;
                    fetch_error = None;
                }
                Err(e) => fetch_error = Some(e),
            }
            last_fetch = Instant::now();
        }

        term.clear();

        let cx = w as usize / 2;
        let mut y = 0;

        let bar_width = (w as usize).min(60);
        let bar_x = cx.saturating_sub(bar_width / 2);

        // Title with pacing indicator right-aligned
        let title = "CLAUDE TOKENS";
        term.set_str(bar_x as i32, y as i32, title, Some(Color::White), true);

        // Pacing indicator on title row, right-aligned
        let five_hour = usage
            .five_hour
            .as_ref()
            .map(|b| b.utilization)
            .unwrap_or(0.0);
        if let Some(ref bucket) = usage.five_hour {
            if let Some(ref resets_at) = bucket.resets_at {
                if let Some(dur) = time_until_reset(resets_at) {
                    let hours_remaining = dur.as_secs_f64() / 3600.0;
                    let hours_elapsed = 5.0 - hours_remaining;
                    // Align so last char falls over the '%' column
                    // '%' is at bar_x + bar_width (end of "{:5.1}%" format)
                    draw_pacing_inline(
                        &mut term,
                        bar_x + bar_width + 1,
                        y,
                        five_hour,
                        hours_elapsed.max(0.0),
                        state.color_scheme(),
                    );
                }
            }
        }
        y += 2;

        // Calculate elapsed percentages for pacing ghost
        let five_hour_expected = usage
            .five_hour
            .as_ref()
            .and_then(|b| b.resets_at.as_ref())
            .and_then(|r| time_until_reset(r))
            .map(|d| elapsed_pct(d, 5.0));

        let seven_day_expected = usage
            .seven_day
            .as_ref()
            .and_then(|b| b.resets_at.as_ref())
            .and_then(|r| time_until_reset(r))
            .map(|d| elapsed_pct(d, 168.0)); // 7 days = 168 hours

        // 5-hour session bar with pacing ghost
        draw_usage_bar(
            &mut term,
            bar_x,
            y,
            bar_width,
            five_hour,
            five_hour_expected,
            "5-Hour",
            &state.colors,
        );
        y += 1;

        // 5-hour reset time
        if let Some(ref bucket) = usage.five_hour {
            if let Some(ref resets_at) = bucket.resets_at {
                if let Some(dur) = time_until_reset(resets_at) {
                    let reset_str = format!("        resets in {}", format_duration(dur));
                    term.set_str(
                        bar_x as i32,
                        y as i32,
                        &reset_str,
                        Some(muted_color_scheme(&state.colors)),
                        false,
                    );
                }
            }
        }
        y += 2;

        // 7-day bar with pacing ghost
        let seven_day_pct = usage
            .seven_day
            .as_ref()
            .map(|b| b.utilization)
            .unwrap_or(0.0);
        draw_usage_bar(
            &mut term,
            bar_x,
            y,
            bar_width,
            seven_day_pct,
            seven_day_expected,
            "7-Day",
            &state.colors,
        );
        y += 1;

        // 7-day reset time
        if let Some(ref bucket) = usage.seven_day {
            if let Some(ref resets_at) = bucket.resets_at {
                if let Some(dur) = time_until_reset(resets_at) {
                    let reset_str = format!("        resets in {}", format_duration(dur));
                    term.set_str(
                        bar_x as i32,
                        y as i32,
                        &reset_str,
                        Some(muted_color_scheme(&state.colors)),
                        false,
                    );
                }
            }
        }
        y += 2;

        // Model-specific bars (no pacing ghost - they share the 7-day window)
        let sonnet_pct = usage
            .seven_day_sonnet
            .as_ref()
            .map(|b| b.utilization)
            .unwrap_or(0.0);
        draw_usage_bar(
            &mut term,
            bar_x,
            y,
            bar_width,
            sonnet_pct,
            seven_day_expected,
            "Sonnet",
            &state.colors,
        );
        y += 1;

        let opus_pct = usage
            .seven_day_opus
            .as_ref()
            .map(|b| b.utilization)
            .unwrap_or(0.0);
        draw_usage_bar(
            &mut term,
            bar_x,
            y,
            bar_width,
            opus_pct,
            seven_day_expected,
            "Opus",
            &state.colors,
        );
        y += 1;

        if let Some(ref err) = fetch_error {
            y += 2;
            let err_str = format!("Error: {}", err);
            let display = &err_str[..err_str.len().min(w as usize)];
            term.set_str(
                (cx - display.len() / 2) as i32,
                y as i32,
                display,
                Some(Color::Red),
                false,
            );
        }

        state.render_help(&mut term, w, h);
        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
