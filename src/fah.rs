use crate::colors::ColorState;
use crate::monitor::layout::{draw_meter_btop_scheme, text_color_scheme, muted_color_scheme, header_color_scheme};
use crate::terminal::Terminal;
use aes::Aes256;
use std::io::Write;

macro_rules! debug_log {
    ($($arg:tt)*) => {{
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/fah_debug.log") {
            let _ = writeln!(f, $($arg)*);
        }
    }};
}
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine;
use crossterm::event::KeyCode;
use crossterm::terminal::size;
use pbkdf2::pbkdf2_hmac;
use rsa::{RsaPrivateKey, Oaep, pkcs8::DecodePrivateKey, pkcs8::EncodePublicKey, traits::PublicKeyParts};
use rsa::pkcs1v15::SigningKey;
use rsa::signature::{Signer, SignatureEncoding};
use serde::Deserialize;
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::TcpStream;
use std::time::{Duration, Instant};
use tungstenite::{connect, Message, WebSocket, stream::MaybeTlsStream};

type Aes256CbcDec = cbc::Decryptor<Aes256>;
type Aes256CbcEnc = cbc::Encryptor<Aes256>;

fn derive_fah_password(email: &str, passphrase: &str) -> String {
    // Step 1: salt = SHA256(email.lower())
    let mut hasher = Sha256::new();
    hasher.update(email.to_lowercase().as_bytes());
    let salt = hasher.finalize();

    // Step 2: key = PBKDF2(passphrase, salt, 100000, SHA-256)
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, 100_000, &mut key);

    // Step 3: hash = SHA256(key)
    let mut hasher = Sha256::new();
    hasher.update(&key);
    let hash = hasher.finalize();

    // Step 4: base64 encode
    base64::engine::general_purpose::STANDARD.encode(hash)
}

// Base64URL encoding/decoding (FAH uses URL-safe base64 without padding for signatures)
fn base64url_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s).ok()
}

// Standard base64 encoding (FAH uses standard base64 for pubkey)
fn base64_std_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn chrono_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    // Convert to ISO 8601 format: 2025-12-14T12:34:56.789Z
    let days_since_epoch = secs / 86400;
    let secs_today = secs % 86400;
    let hours = secs_today / 3600;
    let mins = (secs_today % 3600) / 60;
    let s = secs_today % 60;
    // Simple year/month/day calculation (good enough for ~2020-2100)
    let mut year = 1970;
    let mut remaining_days = days_since_epoch as i64;
    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
        if remaining_days < days_in_year { break; }
        remaining_days -= days_in_year;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_months = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &d in &days_in_months {
        if remaining_days < d { break; }
        remaining_days -= d;
        month += 1;
    }
    let day = remaining_days + 1;
    // Match web client format: include milliseconds
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", year, month, day, hours, mins, s, millis)
}

// Remote WebSocket message types
#[derive(Debug, Deserialize)]
struct WsLoginPayload {
    time: String,
    session: String,
}

#[derive(Debug, Deserialize)]
struct WsConnectMessage {
    #[serde(rename = "type")]
    msg_type: String,
    client: String,
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WsEncryptedMessage {
    #[serde(rename = "type")]
    msg_type: String,
    client: String,
    #[serde(default)]
    session: Option<String>,
    iv: String,
    payload: String,
}

// Decrypted machine state from remote WebSocket
// Full state format: {"session":"...","content":{"info":{...},"units":[{...}]}}
// Delta update format: {"session":"...","content":["units",0,"wu_progress",0.5]}
#[derive(Debug, Deserialize)]
struct RemoteWsUnit {
    wu_progress: Option<f64>,
    ppd: Option<u64>,
    #[serde(default)]
    gpus: Vec<String>,
    assignment: Option<WsAssignment>,
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteWsInfo {
    version: Option<String>,
    os: Option<String>,
    cpu: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteWsContent {
    info: Option<RemoteWsInfo>,
    units: Option<Vec<RemoteWsUnit>>,
}

#[derive(Debug, Deserialize)]
struct RemoteWsState {
    session: Option<String>,
    content: serde_json::Value,  // Can be object (full state) or array (delta update)
}

#[derive(Debug, Deserialize)]
struct FahTeam {
    team: u64,
    name: String,
    score: u64,
}

#[derive(Debug, Deserialize)]
struct FahUser {
    name: String,
    id: u64,
    score: u64,
    wus: u64,
    rank: u64,
    teams: Vec<FahTeam>,
    last: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FahMachine {
    id: String,  // API returns string ID like "KxgiNjSY..."
    name: Option<String>,
    #[serde(default)]
    cpus: u32,
    #[serde(default)]
    gpus: u32,
}

#[derive(Debug, Deserialize)]
struct FahAccount {
    id: Option<String>,  // Account ID
    machines: Option<Vec<FahMachine>>,
}

// WebSocket JSON structures for local FAH client
#[derive(Debug, Deserialize)]
struct WsInfo {
    mach_name: Option<String>,
    cpu_brand: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WsAssignment {
    project: u32,
}

#[derive(Debug, Deserialize)]
struct WsWu {
    run: u32,
    clone: u32,
    gen: u32,
}

#[derive(Debug, Deserialize)]
struct WsUnit {
    state: String,
    cpus: u32,
    #[serde(default)]
    gpus: Vec<String>,
    wu_progress: Option<f64>,
    ppd: Option<u64>,
    eta: Option<String>,
    assignment: Option<WsAssignment>,
    wu: Option<WsWu>,
}

#[derive(Debug, Deserialize)]
struct WsClientData {
    info: Option<WsInfo>,
    units: Option<Vec<WsUnit>>,
}

pub struct FahData {
    pub score: u64,
    pub wus: u64,
    pub rank: u64,
}

#[derive(Default)]
pub struct LocalWorkUnit {
    pub project: u32,
    pub run: u32,
    pub clone: u32,
    pub gen: u32,
    pub progress: f32,
    pub ppd: u64,
    pub eta: String,
    pub state: String,
    pub cpu: String,
    pub threads: u32,
    pub is_gpu: bool,
}

pub struct RemoteWorkUnit {
    pub project: u32,
    pub progress: f32,
    pub ppd: u64,
    pub state: String,
    pub is_gpu: bool,
}

pub struct RemoteMachine {
    pub name: String,
    pub units: Vec<RemoteWorkUnit>,
}

pub struct FahDisplay {
    data: Option<FahData>,
    local_units: Vec<LocalWorkUnit>,
    remote_machines: Vec<RemoteMachine>,
    local_hostname: String,
    local_ws: Option<WebSocket<MaybeTlsStream<TcpStream>>>,
    remote_ws: Option<WebSocket<MaybeTlsStream<TcpStream>>>,
    remote_ws_keys: HashMap<String, Vec<u8>>,  // machine_id -> AES key
    remote_ws_session: Option<String>,  // WebSocket session ID for encrypted messages
    private_key: Option<RsaPrivateKey>,
    remote_machine_ids: Vec<String>,  // Machine IDs from account API
    session_cookie: Option<String>,  // REST API session cookie for WebSocket auth
    fah_sid: Option<String>,  // Persistent session ID from browser localStorage
    error: Option<String>,
}

impl FahDisplay {
    pub fn new() -> Self {
        let local_hostname = fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        Self {
            data: None,
            local_units: Vec::new(),
            remote_machines: Vec::new(),
            local_hostname,
            local_ws: None,
            remote_ws: None,
            remote_ws_keys: HashMap::new(),
            remote_ws_session: None,
            private_key: None,
            remote_machine_ids: Vec::new(),
            session_cookie: None,
            fah_sid: None,
            error: None,
        }
    }

    /// Set persistent session ID from config
    pub fn set_fah_sid(&mut self, sid: Option<String>) {
        self.fah_sid = sid;
    }

    /// Load RSA private key from base64-encoded PKCS#8 format (from browser localStorage fah-secret)
    pub fn load_private_key(&mut self, key_b64: &str) {
        debug_log!("[KEY] Loading private key, {} chars", key_b64.len());
        // Decode base64 and parse PKCS#8
        match base64::engine::general_purpose::STANDARD.decode(key_b64) {
            Ok(der) => {
                debug_log!("[KEY] Decoded {} bytes from fah_secret", der.len());
                match RsaPrivateKey::from_pkcs8_der(&der) {
                    Ok(key) => {
                        debug_log!("[KEY] RSA key loaded: {} bits", key.n().bits());
                        self.private_key = Some(key);
                    }
                    Err(e) => {
                        debug_log!("[KEY] Failed to parse PKCS#8: {:?}", e);
                    }
                }
            }
            Err(e) => {
                debug_log!("[KEY] Failed to decode base64: {:?}", e);
            }
        }
    }

    /// Connect to remote FAH WebSocket relay and authenticate
    pub fn connect_remote_ws(&mut self) {
        let Some(private_key) = self.private_key.clone() else {
            debug_log!("[WS] No private key loaded, skipping remote WS");
            return;
        };
        debug_log!("[WS] Private key loaded, {} bits", private_key.n().bits());

        // Connect to WebSocket with Authorization header (fah_sid)
        let ws_url = "wss://node1.foldingathome.org/ws/account";
        debug_log!("[WS] Connecting to {} with fah_sid: {:?}", ws_url, self.fah_sid.as_ref().map(|s| &s[..s.len().min(20)]));

        // Build request with Authorization header (browser sends fah-sid here too)
        // Include Origin header like the browser does
        let mut request = tungstenite::http::Request::builder()
            .uri(ws_url)
            .header("Host", "node1.foldingathome.org")
            .header("Origin", "https://v8-4.foldingathome.org")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", tungstenite::handshake::client::generate_key());

        // Note: Don't add Authorization header to WebSocket - the login message provides authentication
        // The browser doesn't send Authorization header on WebSocket, only on REST API calls

        let request = match request.body(()) {
            Ok(r) => r,
            Err(_) => return,
        };

        let (mut ws, _) = match connect(request) {
            Ok(r) => {
                debug_log!("[WS] WebSocket connected");
                r
            }
            Err(e) => {
                debug_log!("[WS] WebSocket connection failed: {:?}", e);
                return;
            }
        };

        // Always generate a NEW random session ID for WebSocket (12 random bytes, base64url encoded)
        // Note: fah_sid is for REST API Authorization, NOT for WebSocket session ID
        let mut session_bytes = [0u8; 12];
        for b in &mut session_bytes {
            *b = rand::random();
        }
        let session_id = base64url_encode(&session_bytes);
        debug_log!("[WS] Generated random session ID: {}", session_id);

        // Create login payload
        let time = chrono_now_iso();
        let payload = format!(r#"{{"time":"{}","session":"{}"}}"#, time, session_id);

        // Sign payload with RSA-PKCS1v15-SHA256
        let signing_key = SigningKey::<Sha256>::new(private_key.clone());
        let sig = signing_key.sign(payload.as_bytes());
        let signature = base64url_encode(&sig.to_bytes());

        // Get public key in DER format (SPKI)
        let pubkey = private_key.to_public_key();
        let pubkey_der = match pubkey.to_public_key_der() {
            Ok(der) => der,
            Err(_) => return,
        };
        let pubkey_b64 = base64_std_encode(pubkey_der.as_bytes());

        // Compute account ID: SHA-256 of RSA modulus N, base64url encoded
        // FAH server uses: Digest::urlBase64(key.getRSA_N().toBinString(), "sha256")
        // Web client uses: SHA256(base64_decode(jwk.n))
        let modulus_bytes = pubkey.n().to_bytes_be();
        let mut hasher = Sha256::new();
        hasher.update(&modulus_bytes);
        let account_id = base64url_encode(&hasher.finalize());
        debug_log!("[WS] Our computed account ID: {}", account_id);
        debug_log!("[WS] Our pubkey (first 100 chars): {}", &pubkey_b64[..pubkey_b64.len().min(100)]);

        // Send login message
        let login_msg = format!(
            r#"{{"type":"login","payload":{},"pubkey":"{}","signature":"{}"}}"#,
            payload, pubkey_b64, signature
        );
        debug_log!("[WS] Sending login: payload={}, sig_len={}", payload, signature.len());

        if ws.send(Message::Text(login_msg)).is_err() {
            // eprintln!("[DEBUG] Failed to send login message");
            return;
        }
        // eprintln!("[DEBUG] Login message sent");

        // Set read timeout to receive initial messages (30 seconds to wait for machine connects)
        if let MaybeTlsStream::NativeTls(ref stream) = ws.get_ref() {
            let _ = stream.get_ref().set_read_timeout(Some(std::time::Duration::from_secs(30)));
        }

        // Read initial messages (broadcasts, session connects, machine connects)
        // Machine connects may take time as machines need to detect the new session
        debug_log!("[WS] Waiting for initial messages (30s timeout)...");
        let mut initial_msgs = Vec::new();
        loop {
            match ws.read() {
                Ok(Message::Text(text)) => {
                    debug_log!("[WS INIT] msg {}: {} bytes", initial_msgs.len() + 1, text.len());
                    // Log first 500 chars to see structure
                    debug_log!("[WS INIT] content: {}", &text[..text.len().min(500)]);
                    initial_msgs.push(text);
                }
                Ok(_) => continue,
                Err(e) => {
                    debug_log!("[WS INIT] read done after {} msgs: {:?}", initial_msgs.len(), e);
                    break;
                }
            }
        }

        // Store session ID for encrypted messages
        self.remote_ws_session = Some(session_id);
        self.remote_ws = Some(ws);
        self.remote_ws_keys.clear();

        // Process initial messages (this may trigger session-open sends)
        for text in &initial_msgs {
            self.parse_remote_ws_message(text, &private_key);
        }

        // TEMPORARY: Hardcode AES keys extracted from browser for testing
        // TODO: Fix machine connect flow to derive keys properly
        if self.remote_ws_keys.is_empty() {
            debug_log!("[WS] No keys from machine connects, using hardcoded keys for testing");

            fn hex_to_bytes(s: &str) -> Vec<u8> {
                (0..s.len()).step_by(2)
                    .map(|i| u8::from_str_radix(&s[i..i+2], 16).unwrap())
                    .collect()
            }

            // pk-lintop
            let key1 = hex_to_bytes("cac9560e8a4f7e448a1f14e5599dfc9be96f1f8c4135a64c147e33b96b9ef8b6");
            self.remote_ws_keys.insert("KxgiNjSY3-3El_A2OUQY5oV4SNk1YBh9cSSu-bYGUO8".to_string(), key1);
            // PKWintop
            let key2 = hex_to_bytes("45bafc144556fa844aabb55d364e2b6575f6dd4c87d8bb2002794a4a18d4a970");
            self.remote_ws_keys.insert("aH6iJbamDjGPiuupobkf73ATwbBBUM_oKqkdFdyIYjA".to_string(), key2);
            debug_log!("[WS] Injected 2 hardcoded AES keys");

            // Send session-open to both machines
            for machine_id in self.remote_machine_ids.clone() {
                self.send_session_open(&machine_id);
            }
        }

        // Set non-blocking for ongoing updates
        if let Some(ref ws) = self.remote_ws {
            if let MaybeTlsStream::NativeTls(ref stream) = ws.get_ref() {
                let _ = stream.get_ref().set_nonblocking(true);
            }
        }
        debug_log!("[WS] Remote WebSocket setup complete, {} keys", self.remote_ws_keys.len());
    }

    /// Send encrypted session-open message to a machine
    fn send_session_open(&mut self, machine_id: &str) {
        let Some(ref session_id) = self.remote_ws_session.clone() else {
            debug_log!("[WS] No session ID for session-open");
            return;
        };
        let Some(aes_key) = self.remote_ws_keys.get(machine_id).cloned() else {
            debug_log!("[WS] No AES key for machine {}", machine_id);
            return;
        };

        // Create session-open message: {"type":"session-open","session":"..."}
        let inner_msg = format!(r#"{{"type":"session-open","session":"{}"}}"#, session_id);
        debug_log!("[WS] Sending session-open to {}: {}", machine_id, inner_msg);

        // Encrypt with AES-256-CBC
        self.send_encrypted_to_machine(machine_id, &aes_key, &inner_msg);
    }

    /// Send encrypted message to a machine via the relay
    fn send_encrypted_to_machine(&mut self, machine_id: &str, aes_key: &[u8], plaintext: &str) {
        if aes_key.len() != 32 {
            debug_log!("[WS] Invalid AES key length: {}", aes_key.len());
            return;
        }

        // Generate random IV (16 bytes)
        let mut iv = [0u8; 16];
        for b in &mut iv {
            *b = rand::random();
        }

        // Encrypt with AES-256-CBC PKCS7 padding
        let cipher = Aes256CbcEnc::new(aes_key.into(), &iv.into());
        // Pre-allocate buffer with space for padding (up to 16 bytes)
        let plaintext_bytes = plaintext.as_bytes();
        let mut buf = vec![0u8; plaintext_bytes.len() + 16];
        buf[..plaintext_bytes.len()].copy_from_slice(plaintext_bytes);
        let ciphertext = cipher.encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext_bytes.len())
            .expect("encryption buffer too small");

        // Base64url encode IV and ciphertext
        let iv_b64 = base64url_encode(&iv);
        let payload_b64 = base64url_encode(&ciphertext);

        // Create outer message: {"type":"message","id":"machineId","iv":"...","payload":"..."}
        let outer_msg = format!(
            r#"{{"type":"message","id":"{}","iv":"{}","payload":"{}"}}"#,
            machine_id, iv_b64, payload_b64
        );

        debug_log!("[WS] Sending encrypted message to {}: iv={}, payload_len={}",
            machine_id, iv_b64, payload_b64.len());

        if let Some(ref mut ws) = self.remote_ws {
            if let Err(e) = ws.send(Message::Text(outer_msg)) {
                debug_log!("[WS] Failed to send encrypted message: {:?}", e);
            } else {
                debug_log!("[WS] Encrypted message sent successfully");
            }
        }
    }

    /// Update from remote WebSocket (non-blocking)
    pub fn update_from_remote_ws(&mut self) {
        let Some(ref private_key) = self.private_key.clone() else { return };

        let mut messages = Vec::new();
        let mut disconnected = false;

        if let Some(ws) = &mut self.remote_ws {
            loop {
                match ws.read() {
                    Ok(Message::Text(text)) => {
                        debug_log!("[WS UPDATE] Got message: {} bytes", text.len());
                        messages.push(text);
                    }
                    Ok(_) => continue,
                    Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => {
                        debug_log!("[WS UPDATE] Read error: {:?}", e);
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        // Parse messages
        for text in messages {
            self.parse_remote_ws_message(&text, &private_key);
        }

        if disconnected {
            // eprintln!("[DEBUG] Remote WS disconnected, setting to None");
            self.remote_ws = None;
        }
    }

    fn parse_remote_ws_message(&mut self, text: &str, private_key: &RsaPrivateKey) {
        // Reduce debug spam - only log briefly
        // eprintln!("[WS RAW] {} chars", text.len());

        // Parse the outer message type first
        #[derive(Deserialize)]
        struct MsgType { #[serde(rename = "type")] msg_type: String }
        let msg_type = serde_json::from_str::<MsgType>(text).map(|m| m.msg_type).unwrap_or_default();

        // Log based on type
        match msg_type.as_str() {
            "broadcast" => {
                // Check if it's a state update with machine data
                if text.contains("\"cmd\":\"state\"") && text.contains("\"data\":") {
                    debug_log!("[WS] broadcast state with data: {} bytes", text.len());
                    self.parse_broadcast_state(text);
                } else if text.contains("\"cmd\":\"state\"") {
                    debug_log!("[WS] simple state broadcast");
                } else {
                    debug_log!("[WS] broadcast: {}", &text[..text.len().min(500)]);
                }
                return;
            }
            "connect" => debug_log!("[WS] connect message: {} bytes", text.len()),
            "message" => {
                debug_log!("[WS] encrypted message: {} bytes", text.len());
                debug_log!("[WS] message content: {}", &text[..text.len().min(300)]);
            }
            _ => debug_log!("[WS] unknown type '{}': {} bytes", msg_type, text.len())
        }

        // Parse connect message - machine connections have client object with pubkey/signature/payload
        // Structure: {"type":"connect","client":{"pubkey":"...","signature":"...","payload":{"account":"...","key":"..."}}}
        #[derive(Deserialize, Debug)]
        struct MachineConnectPayload {
            account: Option<String>,
            key: Option<String>,
        }
        #[derive(Deserialize, Debug)]
        struct MachineConnectClient {
            pubkey: Option<String>,
            signature: Option<String>,
            payload: Option<MachineConnectPayload>,
            #[serde(rename = "type")]
            client_type: Option<String>,  // "login" for session notifications
        }
        #[derive(Deserialize, Debug)]
        struct ConnectMessage {
            #[serde(rename = "type")]
            msg_type: String,
            client: serde_json::Value,
        }

        if let Ok(msg) = serde_json::from_str::<ConnectMessage>(text) {
            if msg.msg_type == "connect" {
                debug_log!("[WS] Parsing connect, client value: {}", msg.client);
                // Try to parse client as machine connection object
                if let Ok(client) = serde_json::from_value::<MachineConnectClient>(msg.client.clone()) {
                    debug_log!("[WS] Parsed client: type={:?}, has_pubkey={}, has_payload={}",
                        client.client_type, client.pubkey.is_some(), client.payload.is_some());
                    // Process ALL connect messages (browser doesn't filter by type)
                    // Login sessions will have pubkeys that don't match known machine IDs
                    // Machine connects will have pubkeys that DO match known machine IDs
                    debug_log!("[WS] Processing connect message (type={:?})", client.client_type);

                    // Machine connection - extract key from payload
                    if let Some(payload) = client.payload {
                        if let Some(key_b64) = payload.key {
                            // Compute machine ID from public key (SHA-256 of SPKI DER, base64url encoded)
                            // Browser uses: pubkey_id = SHA256(spki_der_bytes)
                            // The pubkey is in STANDARD base64 (not URL-safe)
                            let machine_id = if let Some(ref pubkey_b64) = client.pubkey {
                                // Try standard base64 first (FAH uses standard for pubkeys)
                                let pubkey_bytes = base64::engine::general_purpose::STANDARD.decode(pubkey_b64)
                                    .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(pubkey_b64));
                                if let Ok(pubkey_bytes) = pubkey_bytes {
                                    let mut hasher = Sha256::new();
                                    hasher.update(&pubkey_bytes);
                                    let hash = hasher.finalize();
                                    base64url_encode(&hash)
                                } else {
                                    debug_log!("[WS] Failed to decode pubkey base64");
                                    "unknown".to_string()
                                }
                            } else {
                                "unknown".to_string()
                            };

                            debug_log!("[WS] Computed connect ID: {}", &machine_id);

                            // Check if this machine ID is one of our known machines
                            let is_known_machine = self.remote_machine_ids.iter().any(|id| id == &machine_id);
                            if !is_known_machine {
                                debug_log!("[WS] Ignoring connect from unknown ID (probably login session)");
                                return;
                            }

                            debug_log!("[WS] *** MACHINE CONNECT from known machine: {} ***", &machine_id);

                            // Try URL-safe base64 first, then standard for the encrypted key
                            let encrypted_key = base64url_decode(&key_b64)
                                .or_else(|| base64::engine::general_purpose::STANDARD.decode(&key_b64).ok());

                            if let Some(encrypted_key) = encrypted_key {
                                debug_log!("[WS] Attempting RSA-OAEP decryption of {} byte key", encrypted_key.len());
                                let padding = Oaep::new::<Sha256>();
                                match private_key.decrypt(padding, &encrypted_key) {
                                    Ok(aes_key) => {
                                        debug_log!("[WS] AES key decrypted: {} bytes for {}", aes_key.len(), &machine_id);
                                        self.remote_ws_keys.insert(machine_id.clone(), aes_key);
                                        // Send session-open to the machine
                                        self.send_session_open(&machine_id);
                                    }
                                    Err(e) => debug_log!("[WS] RSA-OAEP decrypt failed: {:?}", e)
                                }
                            }
                        }
                    }
                }
                return;  // Only return after processing connect messages
            }
        }

        // Try to parse as encrypted message
        if let Ok(msg) = serde_json::from_str::<WsEncryptedMessage>(text) {
            if msg.msg_type == "message" {
                debug_log!("[WS] Encrypted message for client: {}", msg.client);
                if let Some(aes_key) = self.remote_ws_keys.get(&msg.client) {
                    if let (Some(iv), Some(ciphertext)) = (base64url_decode(&msg.iv), base64url_decode(&msg.payload)) {
                        // Decrypt with AES-256-CBC
                        if aes_key.len() == 32 && iv.len() == 16 {
                            let cipher = Aes256CbcDec::new(aes_key.as_slice().into(), iv.as_slice().into());
                            let mut buf = ciphertext;
                            match cipher.decrypt_padded_mut::<Pkcs7>(&mut buf) {
                                Ok(plaintext) => {
                                    debug_log!("[WS] Decrypted {} bytes: {}...", plaintext.len(), String::from_utf8_lossy(&plaintext[..plaintext.len().min(500)]));
                                    match serde_json::from_slice::<RemoteWsState>(plaintext) {
                                        Ok(state) => {
                                            debug_log!("[WS] Parsed state, session: {:?}", state.session);
                                            self.update_remote_machine(&msg.client, state);
                                        }
                                        Err(e) => {
                                            debug_log!("[WS] Failed to parse state JSON: {:?}", e);
                                        }
                                    }
                                }
                                Err(e) => debug_log!("[WS] AES-CBC decrypt failed: {:?}", e)
                            }
                        } else {
                            debug_log!("[WS] Invalid key/IV length: key={}, iv={}", aes_key.len(), iv.len());
                        }
                    }
                } else {
                    debug_log!("[WS] No AES key for client: {}", msg.client);
                }
            }
        }
    }

    fn parse_broadcast_state(&mut self, text: &str) {
        // Parse the broadcast state message
        // Structure: {"type":"broadcast","payload":{"cmd":"state","state":{"machineId":{...},...}}}
        #[derive(Deserialize)]
        struct BroadcastPayload {
            state: Option<HashMap<String, BroadcastMachineState>>,
        }
        #[derive(Deserialize)]
        struct BroadcastMachineState {
            name: Option<String>,
            data: Option<BroadcastMachineData>,
        }
        #[derive(Deserialize)]
        struct BroadcastMachineData {
            units: Option<Vec<BroadcastUnit>>,
        }
        #[derive(Deserialize)]
        struct BroadcastUnit {
            wu_progress: Option<f64>,
            ppd: Option<u64>,
            #[serde(default)]
            gpus: Vec<String>,
            assignment: Option<WsAssignment>,
            state: Option<String>,
        }
        #[derive(Deserialize)]
        struct BroadcastMessage {
            payload: BroadcastPayload,
        }

        match serde_json::from_str::<BroadcastMessage>(text) {
            Ok(msg) => {
                if let Some(state_map) = msg.payload.state {
                    // eprintln!("[DEBUG] Broadcast has {} machines", state_map.len());
                    for (machine_id, machine_state) in state_map {
                        let machine_name = machine_state.name.clone().unwrap_or_else(|| machine_id[..8.min(machine_id.len())].to_string());
                        // eprintln!("[DEBUG] Machine {} ({})", machine_name, machine_id);

                        // Skip local machine (we get that directly)
                        if machine_name.to_lowercase() == self.local_hostname {
                            // eprintln!("[DEBUG] Skipping local machine");
                            continue;
                        }

                        let units: Vec<RemoteWorkUnit> = machine_state.data
                            .and_then(|d| d.units)
                            .unwrap_or_default()
                            .iter()
                            .filter(|u| u.state.as_deref() != Some("finished"))
                            .map(|u| RemoteWorkUnit {
                                project: u.assignment.as_ref().map(|a| a.project).unwrap_or(0),
                                progress: (u.wu_progress.unwrap_or(0.0) * 100.0) as f32,
                                ppd: u.ppd.unwrap_or(0),
                                state: u.state.clone().unwrap_or_default(),
                                is_gpu: !u.gpus.is_empty(),
                            })
                            .collect();

                        // eprintln!("[DEBUG] Machine {} has {} active units", machine_name, units.len());

                        // Update or add machine
                        if let Some(m) = self.remote_machines.iter_mut().find(|m| m.name == machine_name) {
                            m.units = units;
                        } else {
                            self.remote_machines.push(RemoteMachine {
                                name: machine_name,
                                units,
                            });
                        }
                    }
                }
            }
            Err(_) => {} // Failed to parse broadcast state
        }
    }

    fn update_remote_machine(&mut self, machine_id: &str, state: RemoteWsState) {
        // Find machine index by ID in our mapping
        let machine_idx = self.remote_machine_ids.iter().position(|id| id == machine_id);

        // Check if content is a full state object or a delta update array
        if let Some(content_obj) = state.content.as_object() {
            // Full state: {"info":{...},"units":[{...}]}
            if let Some(units_val) = content_obj.get("units") {
                if let Ok(units) = serde_json::from_value::<Vec<RemoteWsUnit>>(units_val.clone()) {
                    let new_units: Vec<RemoteWorkUnit> = units.iter().map(|u| {
                        RemoteWorkUnit {
                            project: u.assignment.as_ref().map(|a| a.project).unwrap_or(0),
                            progress: (u.wu_progress.unwrap_or(0.0) * 100.0) as f32,
                            ppd: u.ppd.unwrap_or(0),
                            state: u.state.clone().unwrap_or_default(),
                            is_gpu: !u.gpus.is_empty(),
                        }
                    }).collect();

                    // Find or create machine
                    if let Some(idx) = machine_idx {
                        if idx < self.remote_machines.len() {
                            self.remote_machines[idx].units = new_units;
                        }
                    } else {
                        // Get machine name from info or use machine_id
                        let name = content_obj.get("info")
                            .and_then(|i| i.get("mach_name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or(machine_id)
                            .to_string();
                        self.remote_machines.push(RemoteMachine {
                            name,
                            units: new_units,
                        });
                        self.remote_machine_ids.push(machine_id.to_string());
                    }
                    debug_log!("[WS] Updated machine {} with {} units", machine_id, units.len());
                }
            }
        } else if let Some(content_arr) = state.content.as_array() {
            // Delta update: ["units", 0, "wu_progress", 0.5]
            if content_arr.len() >= 4 {
                if let (Some("units"), Some(unit_idx), Some(field), Some(value)) = (
                    content_arr.get(0).and_then(|v| v.as_str()),
                    content_arr.get(1).and_then(|v| v.as_u64()).map(|v| v as usize),
                    content_arr.get(2).and_then(|v| v.as_str()),
                    content_arr.get(3),
                ) {
                    // Find the machine by ID and update the specific unit field
                    if let Some(mach_idx) = machine_idx {
                        if mach_idx < self.remote_machines.len() {
                            let machine = &mut self.remote_machines[mach_idx];
                            if machine.units.len() > unit_idx {
                                match field {
                                    "wu_progress" => {
                                        if let Some(progress) = value.as_f64() {
                                            machine.units[unit_idx].progress = (progress * 100.0) as f32;
                                            debug_log!("[WS] Delta: {} unit {} progress = {:.1}%", machine_id, unit_idx, machine.units[unit_idx].progress);
                                        }
                                    }
                                    "ppd" => {
                                        if let Some(ppd) = value.as_u64() {
                                            machine.units[unit_idx].ppd = ppd;
                                        }
                                    }
                                    "state" => {
                                        if let Some(s) = value.as_str() {
                                            machine.units[unit_idx].state = s.to_string();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn connect_local_ws(&mut self) {
        // Connect to local FAH client WebSocket
        match connect("ws://127.0.0.1:7396/api/websocket") {
            Ok((ws, _)) => {
                self.local_ws = Some(ws);
            }
            Err(_) => {
                self.local_ws = None;
            }
        }
    }

    pub fn update_from_local_ws(&mut self) {
        // Collect messages first to avoid borrow checker issues
        let mut messages = Vec::new();
        let mut disconnected = false;

        if let Some(ws) = &mut self.local_ws {
            // Set non-blocking to avoid hanging
            if let MaybeTlsStream::Plain(stream) = ws.get_mut() {
                let _ = stream.set_nonblocking(true);
            }

            // Read available messages
            loop {
                match ws.read() {
                    Ok(Message::Text(text)) => {
                        messages.push(text);
                    }
                    Ok(_) => continue,
                    Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        // Now parse collected messages
        for text in messages {
            self.parse_local_ws_message(&text);
        }

        if disconnected {
            self.local_ws = None;
        }
    }

    fn parse_local_ws_message(&mut self, text: &str) {
        // Try to parse as full client data
        if let Ok(data) = serde_json::from_str::<WsClientData>(text) {
            if let Some(units) = data.units {
                self.local_units.clear();
                let cpu_brand = data.info.as_ref()
                    .and_then(|i| i.cpu_brand.as_ref())
                    .map(|s| {
                        // Shorten CPU name
                        let s = s.replace("Intel(R) Core(TM) ", "")
                            .replace(" CPU", "")
                            .replace(" @ ", " ");
                        if s.len() > 20 {
                            if let Some(space_pos) = s.find(' ') {
                                return s[..space_pos].to_string();
                            }
                        }
                        s
                    })
                    .unwrap_or_default();

                for unit in units {
                    let wu = LocalWorkUnit {
                        project: unit.assignment.as_ref().map(|a| a.project).unwrap_or(0),
                        run: unit.wu.as_ref().map(|w| w.run).unwrap_or(0),
                        clone: unit.wu.as_ref().map(|w| w.clone).unwrap_or(0),
                        gen: unit.wu.as_ref().map(|w| w.gen).unwrap_or(0),
                        progress: (unit.wu_progress.unwrap_or(0.0) * 100.0) as f32,
                        ppd: unit.ppd.unwrap_or(0),
                        eta: unit.eta.unwrap_or_default(),
                        state: unit.state.clone(),
                        cpu: cpu_brand.clone(),
                        threads: unit.cpus,
                        is_gpu: !unit.gpus.is_empty(),
                    };
                    self.local_units.push(wu);
                }
            }
        }
        // Handle incremental updates like ["units", 0, "wu_progress", 0.568]
        else if let Ok(update) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(arr) = update.as_array() {
                if arr.len() >= 4 && arr[0].as_str() == Some("units") {
                    if let Some(idx) = arr[1].as_u64() {
                        let idx = idx as usize;
                        if idx < self.local_units.len() {
                            if let Some(field) = arr[2].as_str() {
                                match field {
                                    "wu_progress" => {
                                        if let Some(v) = arr[3].as_f64() {
                                            self.local_units[idx].progress = (v * 100.0) as f32;
                                        }
                                    }
                                    "ppd" => {
                                        if let Some(v) = arr[3].as_u64() {
                                            self.local_units[idx].ppd = v;
                                        }
                                    }
                                    "eta" => {
                                        if let Some(v) = arr[3].as_str() {
                                            self.local_units[idx].eta = v.to_string();
                                        }
                                    }
                                    "state" => {
                                        if let Some(v) = arr[3].as_str() {
                                            self.local_units[idx].state = v.to_string();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn fetch_stats(&mut self, username: &str) -> io::Result<()> {
        let url = format!("https://api.foldingathome.org/user/{}", username);

        match ureq::get(&url).call() {
            Ok(response) => {
                match response.into_json::<FahUser>() {
                    Ok(user) => {
                        self.data = Some(FahData {
                            score: user.score,
                            wus: user.wus,
                            rank: user.rank,
                        });
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(format!("Parse error: {}", e));
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("Network error: {}", e));
            }
        }

        Ok(())
    }

    /// Fetch remote machines using fah_sid as Authorization header (browser's approach)
    /// Preserves existing unit data on refresh
    pub fn fetch_remote_machines_with_sid(&mut self) {
        let Some(ref fah_sid) = self.fah_sid else {
            debug_log!("[API] No fah_sid available for Authorization");
            return;
        };

        debug_log!("[API] Fetching account with Authorization: {}...", &fah_sid[..fah_sid.len().min(20)]);

        // Use fah_sid as Authorization header (browser's approach)
        let account_resp = match ureq::get("https://api.foldingathome.org/account")
            .set("Authorization", fah_sid)
            .call()
        {
            Ok(r) => r,
            Err(e) => {
                debug_log!("[API] Failed to fetch account with Authorization: {:?}", e);
                return;
            }
        };

        let account: FahAccount = match account_resp.into_json() {
            Ok(a) => a,
            Err(e) => {
                debug_log!("[API] Failed to parse account JSON: {:?}", e);
                return;
            }
        };

        debug_log!("[API] Account ID from REST API: {:?}", account.id);

        let Some(machines) = account.machines else {
            debug_log!("[API] No machines in account");
            return;
        };

        // Only update machine IDs list, preserve existing machine data
        self.remote_machine_ids.clear();
        for machine in &machines {
            self.remote_machine_ids.push(machine.id.clone());
            debug_log!("[API] Machine: {} ({})", machine.name.as_deref().unwrap_or("unnamed"), &machine.id);
        }

        // Only add NEW machines (don't clear existing ones with their units)
        for machine in machines {
            let short_id = if machine.id.len() > 8 { &machine.id[..8] } else { &machine.id };
            let name = machine.name.unwrap_or_else(|| format!("Machine {}", short_id));

            // Check if machine already exists
            let exists = self.remote_machines.iter().any(|m| m.name == name);
            if !exists {
                self.remote_machines.push(RemoteMachine {
                    name,
                    units: Vec::new(), // Will be populated via WebSocket
                });
            }
        }
        debug_log!("[API] Fetched {} machines", self.remote_machine_ids.len());
    }

    pub fn fetch_remote_machines(&mut self, email: &str, passphrase: &str) {
        // Derive password using FAH's PBKDF2 scheme
        let derived_password = derive_fah_password(email, passphrase);

        // Login to get session
        let login_url = format!(
            "https://api.foldingathome.org/login?email={}&password={}",
            urlencoding::encode(email),
            urlencoding::encode(&derived_password)
        );

        let agent = ureq::AgentBuilder::new().build();

        let login_resp = match agent.get(&login_url).call() {
            Ok(r) => r,
            Err(_) => return,
        };

        // Extract session cookie
        let cookie = login_resp
            .header("set-cookie")
            .and_then(|c| c.split(';').next())
            .map(|s| s.to_string());

        let Some(session_cookie) = cookie else { return };

        // Store cookie for WebSocket auth
        self.session_cookie = Some(session_cookie.clone());

        // Fetch account info to get machine list
        let account_resp = match agent
            .get("https://api.foldingathome.org/account")
            .set("Cookie", &session_cookie)
            .call()
        {
            Ok(r) => r,
            Err(_) => return,
        };

        let account: FahAccount = match account_resp.into_json() {
            Ok(a) => a,
            Err(_) => return,
        };

        debug_log!("[API] Account ID from REST API: {:?}", account.id);

        let Some(machines) = account.machines else { return };

        // Store machine IDs for WebSocket subscription
        self.remote_machine_ids.clear();

        // Add NEW machines only (preserve existing ones with their units)
        for machine in machines {
            self.remote_machine_ids.push(machine.id.clone());
            let short_id = if machine.id.len() > 8 { &machine.id[..8] } else { &machine.id };
            let name = machine.name.unwrap_or_else(|| format!("Machine {}", short_id));

            let exists = self.remote_machines.iter().any(|m| m.name == name);
            if !exists {
                self.remote_machines.push(RemoteMachine {
                    name,
                    units: Vec::new(), // Will be populated via WebSocket
                });
            }
        }
    }

    /// Subscribe to remote machines on the relay WebSocket
    /// NOTE: The FAH protocol doesn't use explicit subscription - after login,
    /// the server automatically sends connect messages for your machines.
    pub fn subscribe_to_machines(&mut self) {
        // Disabled: "subscribe" is not a valid FAH message type
        // After successful login, the server should automatically send machine data
    }

    pub fn render(&self, term: &mut Terminal, w: usize, h: usize, colors: &ColorState) {
        let has_stats = self.data.is_some();
        let has_local = !self.local_units.is_empty();
        // Filter out local machine from remote list (avoid duplicates)
        let remote_machines: Vec<_> = self.remote_machines.iter()
            .filter(|m| !has_local || m.name.to_lowercase() != self.local_hostname)
            .collect();
        let has_remote = !remote_machines.is_empty() && remote_machines.iter().any(|m| !m.units.is_empty());

        // Loading state
        if !has_stats && !has_local && !has_remote {
            let cy = h / 2;
            if let Some(ref err) = self.error {
                let msg = format!("Error: {}", err);
                let cx = w.saturating_sub(msg.len()) / 2;
                term.set_str(cx as i32, cy as i32, &msg, Some(header_color_scheme(colors)), false);
            } else {
                let msg = "Connecting to FAH...";
                let cx = w.saturating_sub(msg.len()) / 2;
                term.set_str(cx as i32, cy as i32, msg, Some(text_color_scheme(colors)), false);
            }
            return;
        }

        // Calculate total units for layout
        let local_count = self.local_units.len();
        let remote_count: usize = remote_machines.iter().map(|m| m.units.len()).sum();
        let total_units = local_count + remote_count;

        // Content dimensions (no border)
        let content_width = (w).saturating_sub(4).min(80);
        let content_x = (w.saturating_sub(content_width) / 2) as i32;

        // Calculate content height
        let units_height = total_units;
        let headers_height = if has_local { 1 } else { 0 } + remote_machines.iter().filter(|m| !m.units.is_empty()).count();
        let stats_height = if has_stats { 2 } else { 0 }; // separator + stats
        let content_height = units_height + headers_height + stats_height;

        let mut y = ((h.saturating_sub(content_height)) / 2) as i32;
        let inner_x = content_x;
        let inner_w = content_width;

        // Local section
        if has_local {
            // Section header
            let header = format!(" {} ", self.local_hostname);
            term.set_str(inner_x, y, &header, Some(header_color_scheme(colors)), true);
            y += 1;

            for wu in &self.local_units {
                self.draw_unit_line(term, inner_x, y, inner_w, wu, colors);
                y += 1;
            }
        }

        // Remote sections
        for machine in &remote_machines {
            if machine.units.is_empty() { continue; }

            // Section header
            let header = format!(" {} ", machine.name);
            term.set_str(inner_x, y, &header, Some(header_color_scheme(colors)), true);
            y += 1;

            for unit in &machine.units {
                self.draw_remote_unit_line(term, inner_x, y, inner_w, unit, colors);
                y += 1;
            }
        }

        // Stats line at bottom
        if let Some(ref data) = self.data {
            // Separator
            for i in 0..inner_w {
                term.set(inner_x + i as i32, y, '─', Some(muted_color_scheme(colors)), false);
            }
            y += 1;

            let stats = format!("{} pts · {} WUs · Rank #{}",
                format_number(data.score),
                format_number(data.wus),
                format_number(data.rank)
            );
            let stats_x = inner_x + ((inner_w.saturating_sub(stats.len())) / 2) as i32;
            term.set_str(stats_x, y, &stats, Some(text_color_scheme(colors)), false);
        }
    }

    /// Draw a single work unit on one line (used for both local and remote)
    /// Format: G P12345 ■■■■■■■■■■■■■■■░░░░░  45.2%  1.2M PPD
    fn draw_unit_line(&self, term: &mut Terminal, x: i32, y: i32, width: usize, wu: &LocalWorkUnit, colors: &ColorState) {
        let mut pos = x;

        // C/G indicator for CPU/GPU
        let icon = if wu.is_gpu { "G " } else { "C " };
        term.set_str(pos, y, icon, Some(muted_color_scheme(colors)), false);
        pos += 2;

        // Project ID (7 chars: "P" + 5 digits + space)
        let project = format!("P{:<5} ", wu.project);
        term.set_str(pos, y, &project, Some(text_color_scheme(colors)), false);
        pos += 7;

        // Progress bar - consistent width for all units
        let suffix_len = 20; // " 100.0%  1.2M PPD"
        let bar_width = width.saturating_sub(9 + suffix_len).max(10); // 9 = icon(2) + project(7)

        draw_meter_btop_scheme(term, pos, y, bar_width, wu.progress, colors);
        pos += bar_width as i32;

        // Percentage (7 chars)
        let pct = format!(" {:5.1}%", wu.progress);
        term.set_str(pos, y, &pct, Some(text_color_scheme(colors)), false);
        pos += 7;

        // PPD
        let ppd = format!("  {:>6} PPD", format_ppd(wu.ppd));
        term.set_str(pos, y, &ppd, Some(muted_color_scheme(colors)), false);
    }

    /// Draw a single remote work unit on one line
    fn draw_remote_unit_line(&self, term: &mut Terminal, x: i32, y: i32, width: usize, unit: &RemoteWorkUnit, colors: &ColorState) {
        let mut pos = x;

        // C/G indicator for CPU/GPU
        let icon = if unit.is_gpu { "G " } else { "C " };
        term.set_str(pos, y, icon, Some(muted_color_scheme(colors)), false);
        pos += 2;

        // Project ID
        let project = format!("P{:<5} ", unit.project);
        term.set_str(pos, y, &project, Some(text_color_scheme(colors)), false);
        pos += 7;

        // Progress bar - same width as local
        let suffix_len = 20; // " 100.0%  1.2M PPD"
        let bar_width = width.saturating_sub(9 + suffix_len).max(10); // 9 = icon(2) + project(7)

        draw_meter_btop_scheme(term, pos, y, bar_width, unit.progress, colors);
        pos += bar_width as i32;

        // Percentage
        let pct = format!(" {:5.1}%", unit.progress);
        term.set_str(pos, y, &pct, Some(text_color_scheme(colors)), false);
        pos += 7;

        // PPD
        let ppd = format!("  {:>6} PPD", format_ppd(unit.ppd));
        term.set_str(pos, y, &ppd, Some(muted_color_scheme(colors)), false);
    }

}

fn format_ppd(ppd: u64) -> String {
    if ppd >= 1_000_000 {
        format!("{:.1}M", ppd as f64 / 1_000_000.0)
    } else if ppd >= 1_000 {
        format!("{:.0}K", ppd as f64 / 1_000.0)
    } else {
        format!("{}", ppd)
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

pub struct FahConfig {
    pub username: String,
    pub email: Option<String>,
    pub password: Option<String>,
    pub fah_secret: Option<String>,  // Base64-encoded PKCS#8 RSA private key from browser localStorage
    pub fah_sid: Option<String>,     // Session ID from browser localStorage (fah-sid)
    pub time_step: f32,
}

pub fn run(config: FahConfig) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let mut display = FahDisplay::new();
    let mut colors = ColorState::new(7);

    // Check if we have credentials for remote machines
    let has_creds = config.email.is_some() && config.password.is_some();
    let has_secret = config.fah_secret.is_some();
    let has_sid = config.fah_sid.is_some();
    debug_log!("[INIT] has_creds={}, has_secret={}, has_sid={}", has_creds, has_secret, has_sid);

    // Load RSA private key for encrypted WebSocket if provided
    if let Some(ref secret) = config.fah_secret {
        display.load_private_key(secret);
    }

    // Set persistent session ID if provided (used for REST API Authorization)
    display.set_fah_sid(config.fah_sid.clone());

    // Fetch remote stats
    display.fetch_stats(&config.username)?;

    // Connect to local FAH client WebSocket for real-time updates
    display.connect_local_ws();

    // Fetch remote machines - prefer fah_sid (browser's approach), fall back to email/password
    if has_sid {
        debug_log!("[INIT] Using fah_sid for API authentication (browser approach)");
        display.fetch_remote_machines_with_sid();
    } else if has_creds {
        debug_log!("[INIT] Using email/password for API authentication (legacy)");
        display.fetch_remote_machines(
            config.email.as_ref().unwrap(),
            config.password.as_ref().unwrap(),
        );
    }

    // Connect to remote WebSocket for real-time remote machine updates
    if has_secret {
        display.connect_remote_ws();
        display.subscribe_to_machines();
    }

    let remote_refresh = Duration::from_secs(5 * 60);
    let ws_reconnect = Duration::from_secs(10);
    let remote_ws_reconnect = Duration::from_secs(30);
    let machine_refresh = Duration::from_secs(60);
    let mut last_remote = Instant::now();
    let mut last_ws_check = Instant::now();
    let mut last_remote_ws_check = Instant::now();
    let mut last_machine = Instant::now();

    loop {
        if let Ok(Some((code, _))) = term.check_key() {
            if !colors.handle_key(code) {
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('r') => {
                        display.fetch_stats(&config.username)?;
                        display.connect_local_ws();
                        if has_sid {
                            display.fetch_remote_machines_with_sid();
                        } else if has_creds {
                            display.fetch_remote_machines(
                                config.email.as_ref().unwrap(),
                                config.password.as_ref().unwrap(),
                            );
                        }
                        if has_secret {
                            display.connect_remote_ws();
                            display.subscribe_to_machines();
                        }
                        last_remote = Instant::now();
                        last_ws_check = Instant::now();
                        last_remote_ws_check = Instant::now();
                        last_machine = Instant::now();
                    }
                    _ => {}
                }
            }
        }

        // Update from local WebSocket (non-blocking, real-time updates)
        display.update_from_local_ws();

        // Update from remote WebSocket (non-blocking, real-time remote machine updates)
        display.update_from_remote_ws();

        // Try to reconnect local WebSocket if disconnected
        if display.local_ws.is_none() && last_ws_check.elapsed() >= ws_reconnect {
            display.connect_local_ws();
            last_ws_check = Instant::now();
        }

        // Try to reconnect remote WebSocket if disconnected
        if has_secret && display.remote_ws.is_none() && last_remote_ws_check.elapsed() >= remote_ws_reconnect {
            display.connect_remote_ws();
            display.subscribe_to_machines();
            last_remote_ws_check = Instant::now();
        }

        // Remote refresh every 5 minutes
        if last_remote.elapsed() >= remote_refresh {
            let _ = display.fetch_stats(&config.username);
            last_remote = Instant::now();
        }

        // Machine refresh every 60 seconds
        if (has_sid || has_creds) && last_machine.elapsed() >= machine_refresh {
            if has_sid {
                display.fetch_remote_machines_with_sid();
            } else if has_creds {
                display.fetch_remote_machines(
                    config.email.as_ref().unwrap(),
                    config.password.as_ref().unwrap(),
                );
            }
            last_machine = Instant::now();
        }

        if let Ok((new_w, new_h)) = size() {
            let (cur_w, cur_h) = term.size();
            if new_w != cur_w || new_h != cur_h {
                term.resize(new_w, new_h);
                term.clear_screen()?;
            }
        }

        term.clear();
        let (w, h) = term.size();
        display.render(&mut term, w as usize, h as usize, &colors);
        term.present()?;

        term.sleep(config.time_step);
    }

    Ok(())
}
