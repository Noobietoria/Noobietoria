//! Client kiểm duyệt nội dung (Content Moderation Client) kết nối tới
//! `https://noobie-moderation.noobieeducation.workers.dev/v1/check/confidence-score`
//!
//! Tích hợp bộ nhớ đệm (in-memory + persistent cache) theo hash nội dung
//! để tránh gọi API trùng lặp, tiết kiệm chi phí và tránh spam API.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// URL mặc định của service kiểm duyệt Noobietoria.
pub const DEFAULT_MODERATION_URL: &str =
    "https://noobie-moderation.noobieeducation.workers.dev/v1/check/confidence-score";

/// Lỗi phát sinh trong quá trình kiểm duyệt.
#[derive(Debug, Error)]
pub enum ModerationError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Missing API key")]
    MissingApiKey,
    #[error("API returned status {status}: {message}")]
    ApiFailure { status: u16, message: String },
}

/// Yêu cầu gửi tới API kiểm duyệt.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModerationRequest<'a> {
    text: &'a str,
}

/// Dữ liệu phản hồi kiểm duyệt đã được chuẩn hoá.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationResult {
    /// Nội dung có an toàn để hiển thị không.
    pub is_safe: bool,
    /// Điểm tin cậy / rủi ro (0.0 đến 1.0).
    pub score: f64,
    /// Cờ cảnh báo vi phạm.
    pub flagged: bool,
    /// Nhãn phân loại nếu có.
    pub label: Option<String>,
    /// Kết quả này lấy từ cache hay từ API thật.
    pub cached: bool,
    /// Phản hồi thô từ server.
    pub raw: serde_json::Value,
}

/// Bản ghi trong bộ nhớ cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    result: ModerationResult,
    timestamp: u64,
}

/// Bộ đệm kiểm duyệt nội dung có TTL và lưu trữ file tuỳ chọn.
#[derive(Debug, Clone)]
pub struct ModerationCache {
    entries: Arc<RwLock<BTreeMap<String, CacheEntry>>>,
    cache_file: Option<PathBuf>,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    ttl_seconds: u64,
}

impl Default for ModerationCache {
    fn default() -> Self {
        Self::new(None, 86400 * 7) // Mặc định cache 7 ngày
    }
}

impl ModerationCache {
    /// Tạo cache mới. Nếu có `cache_file`, tự động nạp từ file nếu tồn tại.
    pub fn new(cache_file: Option<PathBuf>, ttl_seconds: u64) -> Self {
        let mut entries = BTreeMap::new();
        if let Some(ref path) = cache_file {
            if path.exists() {
                if let Ok(data) = std::fs::read_to_string(path) {
                    if let Ok(loaded) = serde_json::from_str::<BTreeMap<String, CacheEntry>>(&data)
                    {
                        entries = loaded;
                    }
                }
            }
        }
        Self {
            entries: Arc::new(RwLock::new(entries)),
            cache_file,
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            ttl_seconds,
        }
    }

    /// Tính SHA-256 hash chuẩn hoá của chuỗi văn bản (lowercase, trimmed).
    pub fn hash_text(text: &str) -> String {
        let normalized = text.trim().to_lowercase();
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Lấy kết quả từ cache nếu còn hạn (TTL).
    pub fn get(&self, hash: &str) -> Option<ModerationResult> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entries = self.entries.read();
        if let Some(entry) = entries.get(hash) {
            if self.ttl_seconds == 0 || now.saturating_sub(entry.timestamp) < self.ttl_seconds {
                self.hits.fetch_add(1, Ordering::Relaxed);
                let mut res = entry.result.clone();
                res.cached = true;
                return Some(res);
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Lưu kết quả vào cache và ghi ra đĩa nếu có `cache_file`.
    pub fn insert(&self, hash: String, mut result: ModerationResult) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        result.cached = true;
        let entry = CacheEntry {
            result,
            timestamp: now,
        };

        {
            let mut entries = self.entries.write();
            entries.insert(hash, entry);
        }

        if let Some(ref path) = self.cache_file {
            let snapshot = self.entries.read().clone();
            if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// Số lượng hit cache.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Số lượng miss cache (phải gọi API).
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Tổng số mục đang được lưu trong cache.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Kiểm tra cache có rỗng không.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}

/// Client kiểm duyệt nội dung tích hợp Cache.
#[derive(Clone, Debug)]
pub struct ModerationClient {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    cache: ModerationCache,
    threshold: f64,
}

impl ModerationClient {
    /// Khởi tạo client kiểm duyệt với API key, cache và ngưỡng cảnh báo.
    ///
    /// `threshold`: Ngưỡng điểm số để coi là vi phạm (mặc định 0.70 nếu truyền None).
    pub fn new(
        api_key: impl Into<String>,
        api_url: Option<String>,
        cache_file: Option<PathBuf>,
        threshold: Option<f64>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            client,
            api_url: api_url.unwrap_or_else(|| DEFAULT_MODERATION_URL.to_owned()),
            api_key: api_key.into(),
            cache: ModerationCache::new(cache_file, 86400 * 7),
            threshold: threshold.unwrap_or(0.70),
        }
    }

    /// Tạo client lấy API key từ biến môi trường `NOOBIE_MODERATION_API_KEY`.
    pub fn from_env() -> Self {
        let key = std::env::var("NOOBIE_MODERATION_API_KEY").unwrap_or_default();
        Self::new(key, None, None, None)
    }

    /// Truy cập vào bộ nhớ cache của client.
    pub fn cache(&self) -> &ModerationCache {
        &self.cache
    }

    /// Kiểm duyệt văn bản (bất đồng bộ) kèm cơ chế tra cứu Cache.
    pub async fn check(&self, text: &str) -> Result<ModerationResult, ModerationError> {
        let trimmed = text.trim();
        // Văn bản rỗng thì coi như an toàn, không tốn API call
        if trimmed.is_empty() {
            return Ok(ModerationResult {
                is_safe: true,
                score: 0.0,
                flagged: false,
                label: None,
                cached: true,
                raw: serde_json::json!({ "safe": true, "empty": true }),
            });
        }

        let hash = ModerationCache::hash_text(trimmed);

        // 1. Kiểm tra Cache trước
        if let Some(cached_res) = self.cache.get(&hash) {
            tracing::debug!("Moderation cache hit for hash: {}", hash);
            return Ok(cached_res);
        }

        // 2. Nếu chưa có trong cache và không có API key -> báo lỗi hoặc bypass ở môi trường dev
        if self.api_key.is_empty() {
            tracing::warn!("No moderation API key configured, checking offline heuristics");
            // Offline fallback: kiểm tra các từ tục cơ bản
            let flagged = contains_profanity(trimmed);
            let result = ModerationResult {
                is_safe: !flagged,
                score: if flagged { 1.0 } else { 0.0 },
                flagged,
                label: if flagged {
                    Some("offline_heuristic".to_owned())
                } else {
                    None
                },
                cached: false,
                raw: serde_json::json!({ "offline": true, "flagged": flagged }),
            };
            self.cache.insert(hash, result.clone());
            return Ok(result);
        }

        // 3. Gửi HTTP POST tới Cloudflare Workers endpoint
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Ok(val) = HeaderValue::from_str(&self.api_key) {
            headers.insert("X-API-Key", val);
        }

        let body = ModerationRequest { text: trimmed };

        let response = self
            .client
            .post(&self.api_url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let err_text = response.text().await.unwrap_or_default();
            return Err(ModerationError::ApiFailure {
                status: status.as_u16(),
                message: err_text,
            });
        }

        let raw: serde_json::Value = response.json().await?;

        // Trích xuất score / flagged từ JSON trả về
        let (score, flagged, label) = parse_moderation_payload(&raw, self.threshold);
        let is_safe = !flagged && score < self.threshold;

        let result = ModerationResult {
            is_safe,
            score,
            flagged,
            label,
            cached: false,
            raw,
        };

        // 4. Lưu kết quả vào Cache
        self.cache.insert(hash, result.clone());
        Ok(result)
    }

    /// Phiên bản đồng bộ (blocking) tiện dụng cho context không async.
    pub fn check_blocking(&self, text: &str) -> Result<ModerationResult, ModerationError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(ModerationResult {
                is_safe: true,
                score: 0.0,
                flagged: false,
                label: None,
                cached: true,
                raw: serde_json::json!({ "safe": true, "empty": true }),
            });
        }

        let hash = ModerationCache::hash_text(trimmed);
        if let Some(cached_res) = self.cache.get(&hash) {
            return Ok(cached_res);
        }

        // Dùng tokio current_thread runtime nếu cần
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(self.check(text))),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| ModerationError::Io(std::io::Error::other(e)))?;
                rt.block_on(self.check(text))
            }
        }
    }
}

/// Trích xuất điểm số và cờ cảnh báo từ JSON phản hồi.
fn parse_moderation_payload(
    val: &serde_json::Value,
    threshold: f64,
) -> (f64, bool, Option<String>) {
    // Các dạng format có thể trả về:
    // 1. {"confidence_score": 0.85, "flagged": true}
    // 2. {"score": 0.85, "label": "toxic"}
    // 3. {"result": {"score": 0.85}}
    let score = val
        .get("confidence_score")
        .or_else(|| val.get("score"))
        .or_else(|| val.get("confidence"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let flagged = val
        .get("flagged")
        .and_then(|v| v.as_bool())
        .unwrap_or(score >= threshold);

    let label = val
        .get("label")
        .or_else(|| val.get("category"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    (score, flagged, label)
}

/// Bộ lọc offline dự phòng cơ bản khi không có kết nối internet hoặc API key.
fn contains_profanity(text: &str) -> bool {
    let lower = text.to_lowercase();
    const BLACKLIST: [&str; 6] = ["hack", "cheat", "scam", "phishing", "exploit", "nude"];
    BLACKLIST.iter().any(|bad| lower.contains(bad))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hashing_and_lifecycle() {
        let cache = ModerationCache::default();
        let hash = ModerationCache::hash_text("  Xin chào Noobietoria!  ");
        assert_eq!(hash, ModerationCache::hash_text("xin chào noobietoria!"));

        let res = ModerationResult {
            is_safe: true,
            score: 0.1,
            flagged: false,
            label: None,
            cached: false,
            raw: serde_json::json!({"score": 0.1}),
        };

        assert_eq!(cache.get(&hash).is_none(), true);
        cache.insert(hash.clone(), res);

        let cached = cache.get(&hash).expect("must be in cache");
        assert!(cached.cached);
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.misses(), 1);
    }

    #[test]
    fn empty_text_is_immediately_safe_and_cached() {
        let client = ModerationClient::new("test_key", None, None, None);
        let res = client.check_blocking("   ").unwrap();
        assert!(res.is_safe);
        assert_eq!(res.score, 0.0);
        assert!(res.cached);
    }

    #[test]
    fn offline_fallback_catches_bad_words() {
        let client = ModerationClient::new("", None, None, None);
        let res_bad = client.check_blocking("Free hack tool download").unwrap();
        assert!(!res_bad.is_safe);
        assert!(res_bad.flagged);

        let res_good = client.check_blocking("Welcome to Noobietoria!").unwrap();
        assert!(res_good.is_safe);
        assert!(!res_good.flagged);
    }
}
