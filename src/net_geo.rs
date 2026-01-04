//! Network connection geolocation for globe visualization
//!
//! Parses /proc/net/tcp and /proc/net/udp to get active connections,
//! then uses MaxMind GeoLite2 database to map remote IPs to coordinates.

use maxminddb::{geoip2, Reader};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ============================================================================
// TCP State
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TcpState {
    Established,
    SynSent,
    SynRecv,
    FinWait1,
    FinWait2,
    TimeWait,
    Close,
    CloseWait,
    LastAck,
    Listen,
    Closing,
    Unknown,
}

impl TcpState {
    fn from_hex(hex: &str) -> Self {
        match u8::from_str_radix(hex, 16).unwrap_or(0) {
            0x01 => TcpState::Established,
            0x02 => TcpState::SynSent,
            0x03 => TcpState::SynRecv,
            0x04 => TcpState::FinWait1,
            0x05 => TcpState::FinWait2,
            0x06 => TcpState::TimeWait,
            0x07 => TcpState::Close,
            0x08 => TcpState::CloseWait,
            0x09 => TcpState::LastAck,
            0x0A => TcpState::Listen,
            0x0B => TcpState::Closing,
            _ => TcpState::Unknown,
        }
    }

    /// Priority for display (higher = more important)
    pub fn priority(&self) -> u8 {
        match self {
            TcpState::Established => 4,
            TcpState::SynSent | TcpState::SynRecv => 3,
            TcpState::TimeWait | TcpState::CloseWait => 2,
            TcpState::Listen => 1,
            _ => 0,
        }
    }
}

// ============================================================================
// Protocol
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Protocol {
    Tcp,
    Udp,
}

// ============================================================================
// GeoConnection
// ============================================================================

/// A network connection with geolocation data
#[derive(Clone, Debug)]
#[allow(dead_code)]  // Some fields reserved for future detailed connection views
pub struct GeoConnection {
    pub remote_ip: Ipv4Addr,
    pub remote_port: u16,
    pub local_port: u16,
    pub state: TcpState,
    pub lat: f32,
    pub lon: f32,
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub protocol: Protocol,
    pub is_outbound: bool,  // true = we initiated, false = they connected to us
}

// ============================================================================
// IP Cache
// ============================================================================

/// Cache for IP -> location lookups to avoid repeated database queries
struct IpCache {
    cache: HashMap<Ipv4Addr, Option<(f32, f32)>>,
    max_size: usize,
}

impl IpCache {
    fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size),
            max_size,
        }
    }

    fn get_or_insert<F>(&mut self, ip: Ipv4Addr, lookup_fn: F) -> Option<(f32, f32)>
    where
        F: FnOnce(Ipv4Addr) -> Option<(f32, f32)>,
    {
        if let Some(&cached) = self.cache.get(&ip) {
            return cached;
        }

        // Evict half the cache if at capacity
        if self.cache.len() >= self.max_size {
            let to_remove: Vec<_> = self.cache.keys().take(self.max_size / 2).cloned().collect();
            for key in to_remove {
                self.cache.remove(&key);
            }
        }

        let result = lookup_fn(ip);
        self.cache.insert(ip, result);
        result
    }
}

// ============================================================================
// GeoIP Lookup
// ============================================================================

/// MaxMind GeoLite2 database reader
struct GeoIpLookup {
    reader: Option<Reader<Vec<u8>>>,
}

impl GeoIpLookup {
    fn new(db_path: Option<&Path>) -> Self {
        let reader = Self::find_database(db_path).and_then(|path| {
            Reader::open_readfile(&path).ok()
        });

        Self { reader }
    }

    fn find_database(explicit_path: Option<&Path>) -> Option<PathBuf> {
        // 1. Explicit path from CLI/config
        if let Some(path) = explicit_path {
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }

        // 2. Default locations
        let candidates = [
            dirs::config_dir().map(|p| p.join("termart/GeoLite2-City.mmdb")),
            dirs::config_dir().map(|p| p.join("eDEX-UI/geoIPcache/GeoLite2-City.mmdb")),
            Some(PathBuf::from("/usr/share/GeoIP/GeoLite2-City.mmdb")),
            Some(PathBuf::from("/var/lib/GeoIP/GeoLite2-City.mmdb")),
            Some(PathBuf::from("./GeoLite2-City.mmdb")),
        ];

        candidates.into_iter().flatten().find(|p| p.exists())
    }

    fn lookup(&self, ip: IpAddr) -> Option<(f32, f32)> {
        let reader = self.reader.as_ref()?;
        let city: geoip2::City = reader.lookup(ip).ok()?;
        let location = city.location?;
        let lat = location.latitude? as f32;
        let lon = location.longitude? as f32;
        Some((lat.to_radians(), lon.to_radians()))
    }

    fn is_available(&self) -> bool {
        self.reader.is_some()
    }
}

// ============================================================================
// Connection Tracker
// ============================================================================

/// Tracks network connections and their geolocations
pub struct ConnectionTracker {
    connections: HashMap<(Ipv4Addr, u16, Protocol), GeoConnection>,
    ip_cache: IpCache,
    geo_lookup: GeoIpLookup,
    last_update: Instant,
    update_interval: Duration,
}

impl ConnectionTracker {
    pub fn new(geoip_db: Option<&Path>) -> Self {
        Self {
            connections: HashMap::new(),
            ip_cache: IpCache::new(1024),
            geo_lookup: GeoIpLookup::new(geoip_db),
            last_update: Instant::now() - Duration::from_secs(10), // Force immediate update
            update_interval: Duration::from_secs(2),
        }
    }

    /// Returns true if GeoIP database is loaded
    pub fn has_database(&self) -> bool {
        self.geo_lookup.is_available()
    }

    /// Update connection list from /proc/net
    pub fn update(&mut self) -> io::Result<()> {
        if self.last_update.elapsed() < self.update_interval {
            return Ok(());
        }
        self.last_update = Instant::now();

        let now = Instant::now();
        let mut seen = HashSet::new();

        // Parse TCP connections
        self.parse_proc_net("/proc/net/tcp", Protocol::Tcp, &mut seen, now)?;

        // Parse UDP connections
        self.parse_proc_net("/proc/net/udp", Protocol::Udp, &mut seen, now)?;

        // Remove stale connections
        self.connections.retain(|key, _| seen.contains(key));

        Ok(())
    }

    fn parse_proc_net(
        &mut self,
        path: &str,
        protocol: Protocol,
        seen: &mut HashSet<(Ipv4Addr, u16, Protocol)>,
        now: Instant,
    ) -> io::Result<()> {
        let content = fs::read_to_string(path)?;

        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            // Parse remote address (parts[2])
            let remote = parts[2];
            let Some((rem_ip_hex, rem_port_hex)) = remote.split_once(':') else {
                continue;
            };

            // Skip if no remote connection
            if rem_ip_hex == "00000000" {
                continue;
            }

            let Some(remote_ip) = parse_hex_ip(rem_ip_hex) else {
                continue;
            };

            // Skip localhost and private IPs
            if is_local_or_private(remote_ip) {
                continue;
            }

            // Parse state (TCP only)
            let state = if protocol == Protocol::Tcp {
                TcpState::from_hex(parts[3])
            } else {
                TcpState::Established // UDP is connectionless
            };

            // Skip LISTEN state
            if state == TcpState::Listen {
                continue;
            }

            let remote_port = u16::from_str_radix(rem_port_hex, 16).unwrap_or(0);

            // Parse local port
            let local = parts[1];
            let local_port = local
                .split_once(':')
                .and_then(|(_, p)| u16::from_str_radix(p, 16).ok())
                .unwrap_or(0);

            let key = (remote_ip, remote_port, protocol);
            seen.insert(key);

            // Lookup geolocation (cached)
            let location = self.ip_cache.get_or_insert(remote_ip, |ip| {
                self.geo_lookup.lookup(IpAddr::V4(ip))
            });

            let Some((lat, lon)) = location else {
                continue;
            };

            // Determine direction: ephemeral local port (32768+) = outbound
            let is_outbound = local_port >= 32768;

            // Update or insert connection
            self.connections
                .entry(key)
                .and_modify(|conn| {
                    conn.state = state;
                    conn.last_seen = now;
                })
                .or_insert(GeoConnection {
                    remote_ip,
                    remote_port,
                    local_port,
                    state,
                    lat,
                    lon,
                    first_seen: now,
                    last_seen: now,
                    protocol,
                    is_outbound,
                });
        }

        Ok(())
    }

    /// Iterator over current connections (reserved for future detailed views)
    #[allow(dead_code)]
    pub fn connections(&self) -> impl Iterator<Item = &GeoConnection> {
        self.connections.values()
    }

    /// Get aggregated locations when connection count is high
    pub fn aggregated_locations(&self, max_points: usize) -> Vec<AggregatedLocation> {
        if self.connections.len() <= max_points {
            return self
                .connections
                .values()
                .map(|c| AggregatedLocation {
                    lat: c.lat,
                    lon: c.lon,
                    count: 1,
                    max_state_priority: c.state.priority(),
                    is_outbound: c.is_outbound,
                })
                .collect();
        }

        // Grid-based clustering (5 degree cells)
        // Track outbound count for majority voting
        let mut grid: HashMap<(i32, i32), (AggregatedLocation, usize)> = HashMap::new();
        for conn in self.connections.values() {
            let lat_cell = (conn.lat.to_degrees() / 5.0) as i32;
            let lon_cell = (conn.lon.to_degrees() / 5.0) as i32;
            let key = (lat_cell, lon_cell);

            grid.entry(key)
                .and_modify(|(agg, outbound_count)| {
                    agg.count += 1;
                    agg.max_state_priority = agg.max_state_priority.max(conn.state.priority());
                    if conn.is_outbound {
                        *outbound_count += 1;
                    }
                })
                .or_insert((
                    AggregatedLocation {
                        lat: conn.lat,
                        lon: conn.lon,
                        count: 1,
                        max_state_priority: conn.state.priority(),
                        is_outbound: conn.is_outbound,
                    },
                    if conn.is_outbound { 1 } else { 0 },
                ));
        }

        // Finalize direction based on majority
        let mut result: Vec<_> = grid
            .into_values()
            .map(|(mut agg, outbound_count)| {
                agg.is_outbound = outbound_count > agg.count / 2;
                agg
            })
            .collect();
        result.sort_by(|a, b| {
            b.max_state_priority
                .cmp(&a.max_state_priority)
                .then(b.count.cmp(&a.count))
        });
        result.truncate(max_points);
        result
    }
}

// ============================================================================
// Aggregated Location
// ============================================================================

/// Clustered connection location for high connection counts
#[derive(Clone, Debug)]
pub struct AggregatedLocation {
    pub lat: f32,
    pub lon: f32,
    pub count: usize,
    pub max_state_priority: u8,
    pub is_outbound: bool,  // majority direction in cluster
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse hex IP from /proc/net format (little-endian)
fn parse_hex_ip(hex: &str) -> Option<Ipv4Addr> {
    let bytes = u32::from_str_radix(hex, 16).ok()?;
    Some(Ipv4Addr::new(
        (bytes & 0xFF) as u8,
        ((bytes >> 8) & 0xFF) as u8,
        ((bytes >> 16) & 0xFF) as u8,
        ((bytes >> 24) & 0xFF) as u8,
    ))
}

/// Check if IP is localhost or private range
fn is_local_or_private(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();

    // 127.0.0.0/8 (localhost)
    if octets[0] == 127 {
        return true;
    }

    // 10.0.0.0/8 (private)
    if octets[0] == 10 {
        return true;
    }

    // 172.16.0.0/12 (private)
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }

    // 192.168.0.0/16 (private)
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }

    // 0.0.0.0
    if ip.is_unspecified() {
        return true;
    }

    false
}
