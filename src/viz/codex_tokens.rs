//! Codex usage monitor backed by the local Codex CLI login.

use crate::monitor::layout::muted_color_scheme;
use crate::terminal::Terminal;
use crate::viz::usage::{
    draw_usage_bar, elapsed_percent, format_duration, format_window, text_columns,
};
use crate::viz::VizState;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use serde::Deserialize;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const MAX_BACKOFF_SECS: u64 = 1800;
const HELP: crate::help::HelpSpec = crate::help::HelpSpec::animated(
    "CODEX TOKENS",
    &[crate::help::HelpEntry::new("r", "Refresh now")],
);

#[derive(Deserialize)]
struct AuthFile {
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    access_token: String,
    account_id: String,
}

#[derive(Clone, Default, Deserialize)]
struct UsageResponse {
    plan_type: Option<String>,
    rate_limit: Option<RateLimit>,
    #[serde(default)]
    additional_rate_limits: Vec<AdditionalRateLimit>,
    credits: Option<Credits>,
}

#[derive(Clone, Default, Deserialize)]
struct RateLimit {
    #[serde(default)]
    allowed: bool,
    #[serde(default)]
    limit_reached: bool,
    primary_window: Option<RateLimitWindow>,
    secondary_window: Option<RateLimitWindow>,
}

#[derive(Clone, Default, Deserialize)]
struct RateLimitWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<u64>,
    reset_after_seconds: Option<u64>,
    reset_at: Option<i64>,
}

#[derive(Clone, Default, Deserialize)]
struct AdditionalRateLimit {
    limit_name: Option<String>,
    rate_limit: Option<RateLimit>,
}

#[derive(Clone, Default, Deserialize)]
struct Credits {
    #[serde(default)]
    has_credits: bool,
    #[serde(default)]
    unlimited: bool,
    balance: Option<String>,
}

struct FetchError {
    message: String,
    rate_limited: bool,
}

impl FetchError {
    fn other(message: String) -> Self {
        Self {
            message,
            rate_limited: false,
        }
    }
}

pub struct CodexTokenConfig {
    pub time_step: f32,
    pub refresh_interval: u64,
}

fn auth_path() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(home).join("auth.json"));
    }
    dirs::home_dir().map(|home| home.join(".codex/auth.json"))
}

fn read_auth() -> Result<AuthTokens, String> {
    let path = auth_path().ok_or_else(|| "Cannot locate Codex home directory".to_string())?;
    let data = std::fs::read_to_string(&path)
        .map_err(|error| format!("Cannot read {}: {}", path.display(), error))?;
    serde_json::from_str::<AuthFile>(&data)
        .map_err(|error| format!("Cannot parse {}: {}", path.display(), error))?
        .tokens
        .ok_or_else(|| "No ChatGPT login in Codex auth.json; run 'codex login'".to_string())
}

fn fetch_usage(auth: &AuthTokens) -> Result<UsageResponse, FetchError> {
    let response = ureq::get(USAGE_URL)
        .set("Authorization", &format!("Bearer {}", auth.access_token))
        .set("ChatGPT-Account-Id", &auth.account_id)
        .set("Accept", "application/json")
        .set("User-Agent", "termart-codex-tokens/0.2")
        .call();

    match response {
        Ok(response) => response.into_json().map_err(|error| FetchError {
            message: format!("Invalid usage response: {}", error),
            rate_limited: false,
        }),
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            let detail = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|value| {
                    value
                        .pointer("/detail")
                        .or_else(|| value.pointer("/error/message"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| body.chars().take(120).collect());
            let message = match status {
                401 | 403 => "Codex login expired; run 'codex login'".to_string(),
                _ if detail.is_empty() => format!("HTTP {}", status),
                _ => format!("HTTP {}: {}", status, detail),
            };
            Err(FetchError {
                message,
                rate_limited: status == 429,
            })
        }
        Err(error) => Err(FetchError {
            message: format!("Usage request failed: {}", error),
            rate_limited: false,
        }),
    }
}

fn apply_fetch(
    usage: &mut UsageResponse,
    fetch_error: &mut Option<String>,
    current_interval: &mut Duration,
    base_interval: Duration,
    max_backoff: Duration,
) {
    let result = read_auth()
        .map_err(FetchError::other)
        .and_then(|auth| fetch_usage(&auth));
    match result {
        Ok(response) => {
            *usage = response;
            *fetch_error = None;
            *current_interval = base_interval;
        }
        Err(error) => {
            if error.rate_limited {
                *current_interval = current_interval
                    .saturating_mul(2)
                    .min(max_backoff)
                    .max(base_interval);
                *fetch_error = Some(format!(
                    "{} (retry in {}s)",
                    error.message,
                    current_interval.as_secs()
                ));
            } else {
                *fetch_error = Some(error.message);
            }
        }
    }
}

fn remaining(window: &RateLimitWindow, since_fetch: Duration) -> Option<Duration> {
    if let Some(reset_at) = window.reset_at {
        let now = chrono::Utc::now().timestamp();
        return Some(Duration::from_secs(reset_at.saturating_sub(now) as u64));
    }
    window
        .reset_after_seconds
        .map(Duration::from_secs)
        .map(|duration| duration.saturating_sub(since_fetch))
}

#[allow(clippy::too_many_arguments)]
fn draw_window(
    term: &mut Terminal,
    window: &RateLimitWindow,
    x: usize,
    y: usize,
    width: usize,
    since_fetch: Duration,
    state: &VizState,
    show_reset: bool,
) -> usize {
    let window_duration = Duration::from_secs(window.limit_window_seconds.unwrap_or(0));
    let remaining = remaining(window, since_fetch);
    let expected = remaining.map(|remaining| elapsed_percent(remaining, window_duration));
    let label = format_window(window.limit_window_seconds.unwrap_or(0));

    draw_usage_bar(
        term,
        x,
        y,
        width,
        window.used_percent.unwrap_or(0.0),
        expected,
        &label,
        &state.colors,
    );

    if show_reset {
        if let Some(remaining) = remaining {
            let reset = format!("        resets in {}", format_duration(remaining));
            term.set_str(
                x as i32,
                y as i32 + 1,
                &reset,
                Some(muted_color_scheme(&state.colors)),
                false,
            );
        }
        2
    } else {
        1
    }
}

pub fn run(config: CodexTokenConfig) -> io::Result<()> {
    if let Err(error) = read_auth() {
        eprintln!("Error: {}", error);
        return Ok(());
    }

    let mut term = Terminal::new(true)?;
    let mut state = VizState::new(config.time_step, HELP);
    let base_interval = Duration::from_secs(config.refresh_interval.max(1));
    let max_backoff = Duration::from_secs(MAX_BACKOFF_SECS.max(config.refresh_interval));
    let mut current_interval = base_interval;
    let mut usage = UsageResponse::default();
    let mut fetch_error = None;
    let mut last_fetch = Instant::now();

    apply_fetch(
        &mut usage,
        &mut fetch_error,
        &mut current_interval,
        base_interval,
        max_backoff,
    );

    let (mut width, mut height) = term.size();
    loop {
        if let Ok(Some((code, modifiers))) = term.check_key() {
            if state.handle_key(code, modifiers) {
                break;
            }
            if matches!(code, KeyCode::Char('r') | KeyCode::Char('R')) {
                apply_fetch(
                    &mut usage,
                    &mut fetch_error,
                    &mut current_interval,
                    base_interval,
                    max_backoff,
                );
                last_fetch = Instant::now();
            }
        }

        if let Ok((new_width, new_height)) = size() {
            if new_width != width || new_height != height {
                width = new_width;
                height = new_height;
                term.resize(width, height);
                term.clear_screen()?;
            }
        }

        if last_fetch.elapsed() >= current_interval {
            apply_fetch(
                &mut usage,
                &mut fetch_error,
                &mut current_interval,
                base_interval,
                max_backoff,
            );
            last_fetch = Instant::now();
        }

        term.clear();
        let screen_width = width as usize;
        let bar_width = screen_width.min(60);
        let bar_x = screen_width.saturating_sub(bar_width) / 2;
        let mut y = 0usize;

        term.set_str(
            bar_x as i32,
            y as i32,
            "CODEX TOKENS",
            Some(Color::White),
            true,
        );
        let status = if usage
            .rate_limit
            .as_ref()
            .is_some_and(|limit| limit.limit_reached || !limit.allowed)
        {
            "LIMIT REACHED".to_string()
        } else {
            usage
                .plan_type
                .as_deref()
                .unwrap_or("unknown")
                .to_uppercase()
        };
        if text_columns(&status) + text_columns("CODEX TOKENS") + 2 <= bar_width {
            term.set_str(
                (bar_x + bar_width - text_columns(&status)) as i32,
                y as i32,
                &status,
                Some(if status == "LIMIT REACHED" {
                    Color::Red
                } else {
                    muted_color_scheme(&state.colors)
                }),
                status == "LIMIT REACHED",
            );
        }
        y += 2;

        let since_fetch = last_fetch.elapsed();
        if let Some(limit) = usage.rate_limit.as_ref() {
            if let Some(window) = limit.primary_window.as_ref() {
                y += draw_window(
                    &mut term,
                    window,
                    bar_x,
                    y,
                    bar_width,
                    since_fetch,
                    &state,
                    true,
                ) + 1;
            }
            if let Some(window) = limit.secondary_window.as_ref() {
                y += draw_window(
                    &mut term,
                    window,
                    bar_x,
                    y,
                    bar_width,
                    since_fetch,
                    &state,
                    true,
                ) + 1;
            }
        }

        // Additional model-specific quotas are useful, but only render them
        // when the pane can contain the whole three-row group.
        for additional in &usage.additional_rate_limits {
            if y + 3 > height as usize {
                break;
            }
            let name = additional
                .limit_name
                .as_deref()
                .unwrap_or("Additional limit");
            let name: String = name.chars().take(bar_width).collect();
            term.set_str(
                bar_x as i32,
                y as i32,
                &name,
                Some(muted_color_scheme(&state.colors)),
                true,
            );
            y += 1;
            if let Some(limit) = additional.rate_limit.as_ref() {
                if let Some(window) = limit.primary_window.as_ref() {
                    y += draw_window(
                        &mut term,
                        window,
                        bar_x,
                        y,
                        bar_width,
                        since_fetch,
                        &state,
                        false,
                    );
                }
                if let Some(window) = limit.secondary_window.as_ref() {
                    y += draw_window(
                        &mut term,
                        window,
                        bar_x,
                        y,
                        bar_width,
                        since_fetch,
                        &state,
                        false,
                    );
                }
            }
        }

        if let Some(credits) = usage.credits.as_ref() {
            if y < height as usize && (credits.has_credits || credits.unlimited) {
                let credit_text = if credits.unlimited {
                    "Credits: unlimited".to_string()
                } else {
                    format!(
                        "Credits: {}",
                        credits.balance.as_deref().unwrap_or("available")
                    )
                };
                term.set_str(
                    bar_x as i32,
                    y as i32,
                    &credit_text,
                    Some(muted_color_scheme(&state.colors)),
                    false,
                );
            }
        }

        if let Some(error) = fetch_error.as_ref() {
            let message = format!("Error: {}", error);
            let message: String = message.chars().take(screen_width).collect();
            let error_y = (height as usize).saturating_sub(1);
            term.set_str(
                screen_width.saturating_sub(text_columns(&message)) as i32 / 2,
                error_y as i32,
                &message,
                Some(Color::Red),
                false,
            );
        }

        state.render_help(&mut term, width, height);
        term.present()?;
        term.sleep(state.speed);
    }

    Ok(())
}
