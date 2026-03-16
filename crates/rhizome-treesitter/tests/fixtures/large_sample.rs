//! A large sample Rust file for benchmarking tree-sitter parsing.
//! This file contains ~1000 lines of realistic Rust code.

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// ───────────────────────────────── Constants ─────────────────────────────────

const MAX_CONNECTIONS: usize = 1024;
const DEFAULT_PORT: u16 = 8080;
const BUFFER_SIZE: usize = 4096;
const TIMEOUT_MS: u64 = 30_000;
const VERSION: &str = "0.1.0";

// ───────────────────────────────── Enums ─────────────────────────────────────

/// HTTP methods supported by the server.
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

/// Possible server states.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerState {
    Starting,
    Running,
    Paused,
    ShuttingDown,
    Stopped,
}

/// Log levels for the logging system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

/// Database connection pool status.
pub enum PoolStatus {
    Healthy,
    Degraded { available: usize, total: usize },
    Exhausted,
    Error(String),
}

/// Cache eviction strategy.
pub enum EvictionPolicy {
    Lru,
    Lfu,
    Fifo,
    Random,
    Ttl { max_age_secs: u64 },
}

/// Authentication result.
pub enum AuthResult {
    Authenticated(UserId),
    Denied { reason: String },
    Expired,
    MfaRequired,
}

/// Middleware execution result.
pub enum MiddlewareAction {
    Continue,
    Halt(Response),
    Redirect(String),
}

/// Compression algorithms.
pub enum Compression {
    None,
    Gzip,
    Brotli,
    Zstd { level: i32 },
}

// ───────────────────────────────── Structs ────────────────────────────────────

/// Unique user identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(pub u64);

/// Configuration for the HTTP server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub read_timeout_ms: u64,
    pub write_timeout_ms: u64,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
}

/// An HTTP request.
pub struct Request {
    pub method: HttpMethod,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub query_params: HashMap<String, String>,
}

/// An HTTP response.
pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// A route handler entry.
pub struct Route {
    pub method: HttpMethod,
    pub pattern: String,
    pub handler: Box<dyn Fn(&Request) -> Response + Send + Sync>,
}

/// The main HTTP server.
pub struct HttpServer {
    config: ServerConfig,
    routes: Vec<Route>,
    state: ServerState,
    middleware: Vec<Box<dyn Middleware>>,
    logger: Logger,
}

/// A database connection.
pub struct DbConnection {
    host: String,
    port: u16,
    database: String,
    username: String,
    connected: bool,
}

/// Connection pool for database access.
pub struct ConnectionPool {
    connections: Vec<DbConnection>,
    max_size: usize,
    min_idle: usize,
    timeout_ms: u64,
}

/// In-memory cache with configurable eviction.
pub struct Cache<K, V> {
    entries: HashMap<K, CacheEntry<V>>,
    max_entries: usize,
    policy: EvictionPolicy,
    hits: u64,
    misses: u64,
}

/// A single cache entry with metadata.
struct CacheEntry<V> {
    value: V,
    created_at: u64,
    last_accessed: u64,
    access_count: u64,
}

/// Rate limiter using token bucket algorithm.
pub struct RateLimiter {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: u64,
}

/// Session data for authenticated users.
pub struct Session {
    pub id: String,
    pub user_id: UserId,
    pub created_at: u64,
    pub expires_at: u64,
    pub data: HashMap<String, String>,
}

/// Logger with configurable output.
pub struct Logger {
    level: LogLevel,
    prefix: String,
    output: LogOutput,
}

/// Where log messages are sent.
pub enum LogOutput {
    Stdout,
    Stderr,
    File(PathBuf),
}

/// Metrics collector for monitoring.
pub struct MetricsCollector {
    counters: HashMap<String, u64>,
    gauges: HashMap<String, f64>,
    histograms: HashMap<String, Vec<f64>>,
}

/// Template engine for rendering HTML.
pub struct TemplateEngine {
    templates: HashMap<String, String>,
    cache_compiled: bool,
}

/// JSON Web Token claims.
pub struct JwtClaims {
    pub sub: String,
    pub exp: u64,
    pub iat: u64,
    pub roles: Vec<String>,
}

/// Health check result.
pub struct HealthCheck {
    pub status: String,
    pub uptime_secs: u64,
    pub checks: Vec<ComponentHealth>,
}

/// Individual component health.
pub struct ComponentHealth {
    pub name: String,
    pub healthy: bool,
    pub message: Option<String>,
    pub latency_ms: Option<u64>,
}

// ───────────────────────────────── Traits ─────────────────────────────────────

/// Middleware trait for request/response processing.
pub trait Middleware: Send + Sync {
    fn before(&self, request: &Request) -> MiddlewareAction;
    fn after(&self, request: &Request, response: &mut Response);
}

/// Serialization trait.
pub trait Serialize {
    fn to_json(&self) -> String;
    fn to_bytes(&self) -> Vec<u8>;
}

/// Deserialization trait.
pub trait Deserialize: Sized {
    fn from_json(json: &str) -> Result<Self, String>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, String>;
}

/// Repository pattern for data access.
pub trait Repository<T> {
    fn find_by_id(&self, id: u64) -> Option<T>;
    fn find_all(&self) -> Vec<T>;
    fn create(&mut self, item: T) -> u64;
    fn update(&mut self, id: u64, item: T) -> bool;
    fn delete(&mut self, id: u64) -> bool;
}

/// Event handler trait.
pub trait EventHandler {
    fn handle(&self, event: &str, payload: &[u8]);
    fn event_types(&self) -> Vec<String>;
}

/// Authentication provider.
pub trait AuthProvider {
    fn authenticate(&self, token: &str) -> AuthResult;
    fn refresh(&self, session: &Session) -> Option<Session>;
    fn revoke(&self, session_id: &str);
}

/// Validator trait for input validation.
pub trait Validator {
    fn validate(&self) -> Vec<ValidationError>;
    fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

/// Compression provider.
pub trait Compressor {
    fn compress(&self, data: &[u8]) -> Vec<u8>;
    fn decompress(&self, data: &[u8]) -> Vec<u8>;
    fn algorithm(&self) -> Compression;
}

/// Health check provider.
pub trait HealthCheckable {
    fn check_health(&self) -> ComponentHealth;
}

// ───────────────────────────────── Validation ────────────────────────────────

/// A validation error.
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
}

// ───────────────────────────────── Implementations ───────────────────────────

impl ServerConfig {
    /// Create a new server configuration with defaults.
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            max_connections: MAX_CONNECTIONS,
            read_timeout_ms: TIMEOUT_MS,
            write_timeout_ms: TIMEOUT_MS,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }

    /// Check if TLS is configured.
    pub fn has_tls(&self) -> bool {
        self.tls_cert_path.is_some() && self.tls_key_path.is_some()
    }

    /// Return the full address string.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new("127.0.0.1".to_string(), DEFAULT_PORT)
    }
}

impl Request {
    /// Create a new GET request.
    pub fn get(path: &str) -> Self {
        Self {
            method: HttpMethod::Get,
            path: path.to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
            query_params: HashMap::new(),
        }
    }

    /// Add a header to the request.
    pub fn with_header(self, key: &str, value: &str) -> Self {
        let mut headers = self.headers;
        headers.insert(key.to_string(), value.to_string());
        Self { headers, ..self }
    }

    /// Add a query parameter.
    pub fn with_query(self, key: &str, value: &str) -> Self {
        let mut params = self.query_params;
        params.insert(key.to_string(), value.to_string());
        Self {
            query_params: params,
            ..self
        }
    }

    /// Get a header value.
    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(|s| s.as_str())
    }

    /// Get the content type header.
    pub fn content_type(&self) -> Option<&str> {
        self.header("Content-Type")
    }

    /// Check if this is a JSON request.
    pub fn is_json(&self) -> bool {
        self.content_type()
            .map(|ct| ct.contains("application/json"))
            .unwrap_or(false)
    }
}

impl Response {
    /// Create a 200 OK response.
    pub fn ok(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            headers: HashMap::new(),
            body,
        }
    }

    /// Create a 404 Not Found response.
    pub fn not_found() -> Self {
        Self {
            status: 404,
            headers: HashMap::new(),
            body: b"Not Found".to_vec(),
        }
    }

    /// Create a 500 Internal Server Error response.
    pub fn internal_error(msg: &str) -> Self {
        Self {
            status: 500,
            headers: HashMap::new(),
            body: msg.as_bytes().to_vec(),
        }
    }

    /// Create a JSON response.
    pub fn json(status: u16, body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        Self {
            status,
            headers,
            body: body.as_bytes().to_vec(),
        }
    }

    /// Add a header to the response.
    pub fn with_header(self, key: &str, value: &str) -> Self {
        let mut headers = self.headers;
        headers.insert(key.to_string(), value.to_string());
        Self { headers, ..self }
    }
}

impl HttpServer {
    /// Create a new HTTP server with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            routes: Vec::new(),
            state: ServerState::Stopped,
            middleware: Vec::new(),
            logger: Logger::new(LogLevel::Info),
        }
    }

    /// Register a route handler.
    pub fn route(&mut self, method: HttpMethod, pattern: &str, handler: impl Fn(&Request) -> Response + Send + Sync + 'static) {
        self.routes.push(Route {
            method,
            pattern: pattern.to_string(),
            handler: Box::new(handler),
        });
    }

    /// Add middleware to the processing pipeline.
    pub fn use_middleware(&mut self, mw: Box<dyn Middleware>) {
        self.middleware.push(mw);
    }

    /// Start the server.
    pub fn start(&mut self) -> io::Result<()> {
        self.state = ServerState::Starting;
        self.logger.info(&format!("Starting server on {}", self.config.address()));
        self.state = ServerState::Running;
        Ok(())
    }

    /// Stop the server gracefully.
    pub fn stop(&mut self) {
        self.state = ServerState::ShuttingDown;
        self.logger.info("Shutting down server");
        self.state = ServerState::Stopped;
    }

    /// Get the current server state.
    pub fn state(&self) -> &ServerState {
        &self.state
    }

    /// Handle an incoming request through the middleware chain and router.
    pub fn handle_request(&self, request: &Request) -> Response {
        for mw in &self.middleware {
            match mw.before(request) {
                MiddlewareAction::Continue => {}
                MiddlewareAction::Halt(response) => return response,
                MiddlewareAction::Redirect(url) => {
                    return Response::ok(url.into_bytes())
                        .with_header("Location", &url);
                }
            }
        }

        let mut response = self.route_request(request);

        for mw in self.middleware.iter().rev() {
            mw.after(request, &mut response);
        }

        response
    }

    fn route_request(&self, request: &Request) -> Response {
        for route in &self.routes {
            if route.pattern == request.path {
                return (route.handler)(request);
            }
        }
        Response::not_found()
    }
}

impl DbConnection {
    /// Create a new database connection.
    pub fn new(host: &str, port: u16, database: &str, username: &str) -> Self {
        Self {
            host: host.to_string(),
            port,
            database: database.to_string(),
            username: username.to_string(),
            connected: false,
        }
    }

    /// Connect to the database.
    pub fn connect(&mut self) -> Result<(), String> {
        self.connected = true;
        Ok(())
    }

    /// Disconnect from the database.
    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Execute a query.
    pub fn execute(&self, query: &str) -> Result<Vec<HashMap<String, String>>, String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }
        let _ = query;
        Ok(Vec::new())
    }
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub fn new(max_size: usize) -> Self {
        Self {
            connections: Vec::new(),
            max_size,
            min_idle: 2,
            timeout_ms: 5000,
        }
    }

    /// Get a connection from the pool.
    pub fn get(&mut self) -> Option<&mut DbConnection> {
        self.connections.iter_mut().find(|c| !c.is_connected())
    }

    /// Return the pool status.
    pub fn status(&self) -> PoolStatus {
        let available = self.connections.iter().filter(|c| !c.is_connected()).count();
        if available == 0 && self.connections.len() >= self.max_size {
            PoolStatus::Exhausted
        } else if available < self.min_idle {
            PoolStatus::Degraded {
                available,
                total: self.connections.len(),
            }
        } else {
            PoolStatus::Healthy
        }
    }

    /// Get pool size.
    pub fn size(&self) -> usize {
        self.connections.len()
    }
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Cache<K, V> {
    /// Create a new cache.
    pub fn new(max_entries: usize, policy: EvictionPolicy) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            policy,
            hits: 0,
            misses: 0,
        }
    }

    /// Get a value from the cache.
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.get(key) {
            self.hits += 1;
            Some(entry.value.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert a value into the cache.
    pub fn insert(&mut self, key: K, value: V) {
        if self.entries.len() >= self.max_entries {
            self.evict();
        }
        self.entries.insert(
            key,
            CacheEntry {
                value,
                created_at: 0,
                last_accessed: 0,
                access_count: 0,
            },
        );
    }

    /// Get the hit rate as a percentage.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }

    /// Number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn evict(&mut self) {
        if let Some(key) = self.entries.keys().next().cloned() {
            self.entries.remove(&key);
        }
    }
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: 0,
        }
    }

    /// Try to acquire a token. Returns true if allowed.
    pub fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Get remaining tokens.
    pub fn remaining(&self) -> f64 {
        self.tokens
    }

    fn refill(&mut self) {
        self.tokens = (self.tokens + self.refill_rate).min(self.max_tokens);
    }
}

impl Logger {
    /// Create a new logger.
    pub fn new(level: LogLevel) -> Self {
        Self {
            level,
            prefix: String::new(),
            output: LogOutput::Stderr,
        }
    }

    /// Create a logger with a prefix.
    pub fn with_prefix(level: LogLevel, prefix: &str) -> Self {
        Self {
            level,
            prefix: prefix.to_string(),
            output: LogOutput::Stderr,
        }
    }

    /// Log an info message.
    pub fn info(&self, msg: &str) {
        if self.level <= LogLevel::Info {
            self.write(LogLevel::Info, msg);
        }
    }

    /// Log a warning message.
    pub fn warn(&self, msg: &str) {
        if self.level <= LogLevel::Warn {
            self.write(LogLevel::Warn, msg);
        }
    }

    /// Log an error message.
    pub fn error(&self, msg: &str) {
        if self.level <= LogLevel::Error {
            self.write(LogLevel::Error, msg);
        }
    }

    /// Log a debug message.
    pub fn debug(&self, msg: &str) {
        if self.level <= LogLevel::Debug {
            self.write(LogLevel::Debug, msg);
        }
    }

    fn write(&self, level: LogLevel, msg: &str) {
        let _ = (level, msg, &self.prefix);
    }
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
            gauges: HashMap::new(),
            histograms: HashMap::new(),
        }
    }

    /// Increment a counter.
    pub fn increment(&mut self, name: &str) {
        *self.counters.entry(name.to_string()).or_insert(0) += 1;
    }

    /// Set a gauge value.
    pub fn gauge(&mut self, name: &str, value: f64) {
        self.gauges.insert(name.to_string(), value);
    }

    /// Record a histogram observation.
    pub fn observe(&mut self, name: &str, value: f64) {
        self.histograms
            .entry(name.to_string())
            .or_default()
            .push(value);
    }

    /// Get a counter value.
    pub fn get_counter(&self, name: &str) -> u64 {
        self.counters.get(name).copied().unwrap_or(0)
    }

    /// Get a gauge value.
    pub fn get_gauge(&self, name: &str) -> Option<f64> {
        self.gauges.get(name).copied()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine {
    /// Create a new template engine.
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            cache_compiled: true,
        }
    }

    /// Register a template.
    pub fn register(&mut self, name: &str, template: &str) {
        self.templates.insert(name.to_string(), template.to_string());
    }

    /// Render a template with variables.
    pub fn render(&self, name: &str, vars: &HashMap<String, String>) -> Result<String, String> {
        let template = self
            .templates
            .get(name)
            .ok_or_else(|| format!("Template not found: {}", name))?;

        let mut output = template.clone();
        for (key, value) in vars {
            output = output.replace(&format!("{{{{{}}}}}", key), value);
        }
        Ok(output)
    }

    /// Check if a template is registered.
    pub fn has_template(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ServerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerState::Starting => write!(f, "starting"),
            ServerState::Running => write!(f, "running"),
            ServerState::Paused => write!(f, "paused"),
            ServerState::ShuttingDown => write!(f, "shutting_down"),
            ServerState::Stopped => write!(f, "stopped"),
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

// ───────────────────────────────── Modules ────────────────────────────────────

mod utils {
    /// Parse a URL-encoded query string.
    pub fn parse_query_string(query: &str) -> Vec<(String, String)> {
        query
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                let key = parts.next()?.to_string();
                let value = parts.next().unwrap_or("").to_string();
                Some((key, value))
            })
            .collect()
    }

    /// Percent-decode a string.
    pub fn percent_decode(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars();
        while let Some(c) = chars.next() {
            if c == '%' {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                }
            } else if c == '+' {
                result.push(' ');
            } else {
                result.push(c);
            }
        }
        result
    }

    /// Escape HTML entities.
    pub fn escape_html(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }

    /// Generate a simple hash for a string.
    pub fn simple_hash(input: &str) -> u64 {
        let mut hash: u64 = 5381;
        for byte in input.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
        }
        hash
    }
}

mod crypto {
    /// Constant-time comparison of two byte slices.
    pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut result = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            result |= x ^ y;
        }
        result == 0
    }

    /// Simple base64 encode (not production quality).
    pub fn base64_encode(input: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = String::new();
        for chunk in input.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let triple = (b0 << 16) | (b1 << 8) | b2;
            result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
            result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 {
                result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }
            if chunk.len() > 2 {
                result.push(ALPHABET[(triple & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }
        }
        result
    }
}

// ───────────────────────────────── Free Functions ─────────────────────────────

/// Parse a header line into key-value pair.
pub fn parse_header(line: &str) -> Option<(String, String)> {
    let mut parts = line.splitn(2, ':');
    let key = parts.next()?.trim().to_string();
    let value = parts.next()?.trim().to_string();
    Some((key, value))
}

/// Format a file size for display.
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} PB", size)
}

/// Slugify a string for URLs.
pub fn slugify(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Truncate a string with ellipsis.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s[..max_len].to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Calculate the Levenshtein distance between two strings.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

/// Validate an email address (simple check).
pub fn is_valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }
    let local = parts[0];
    let domain = parts[1];
    !local.is_empty() && !domain.is_empty() && domain.contains('.')
}

/// Generate a simple UUID-like string.
pub fn generate_id() -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        0u32, 0u16, 0u16, 0u16, 0u64
    )
}

/// Pluralize a word based on count.
pub fn pluralize(word: &str, count: usize) -> String {
    if count == 1 {
        word.to_string()
    } else {
        format!("{}s", word)
    }
}

/// Extract file extension from a path.
pub fn file_extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|e| e.to_str())
}

/// Check if a string is a valid identifier.
pub fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    (first.is_alphabetic() || first == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Wrap text to a maximum line width.
pub fn word_wrap(text: &str, max_width: usize) -> String {
    let mut result = String::new();
    let mut line_len = 0;

    for word in text.split_whitespace() {
        if line_len + word.len() + 1 > max_width && line_len > 0 {
            result.push('\n');
            line_len = 0;
        } else if line_len > 0 {
            result.push(' ');
            line_len += 1;
        }
        result.push_str(word);
        line_len += word.len();
    }
    result
}

/// Convert camelCase to snake_case.
pub fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}
