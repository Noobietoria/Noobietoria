/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Universal Blob Storage: supports Local Filesystem, AWS S3, Cloudflare R2, and MinIO.

use std::path::PathBuf;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O storage error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network/HTTP storage error: {0}")]
    Http(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Abstract persistent object/blob storage for place files, assets, and textures.
pub trait BlobStorage: Send + Sync + std::fmt::Debug {
    fn put<'a>(
        &'a self,
        path: &'a str,
        data: &'a [u8],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StorageError>> + Send + 'a>>;

    fn get<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<Vec<u8>>, StorageError>> + Send + 'a>,
    >;

    fn delete<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, StorageError>> + Send + 'a>>;

    fn exists<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, StorageError>> + Send + 'a>>;
}

// ── Local Filesystem Backend ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LocalStorage {
    root_dir: PathBuf,
}

impl LocalStorage {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        let root = root_dir.into();
        let _ = std::fs::create_dir_all(&root);
        Self { root_dir: root }
    }

    fn resolve_path(&self, rel: &str) -> PathBuf {
        let safe_rel = rel.trim_start_matches('/').trim_start_matches('\\');
        self.root_dir.join(safe_rel)
    }
}

impl BlobStorage for LocalStorage {
    fn put<'a>(
        &'a self,
        path: &'a str,
        data: &'a [u8],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StorageError>> + Send + 'a>>
    {
        Box::pin(async move {
            let full_path = self.resolve_path(path);
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&full_path, data).await?;
            Ok(())
        })
    }

    fn get<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<Vec<u8>>, StorageError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let full_path = self.resolve_path(path);
            match tokio::fs::read(&full_path).await {
                Ok(bytes) => Ok(Some(bytes)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(StorageError::Io(e)),
            }
        })
    }

    fn delete<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, StorageError>> + Send + 'a>>
    {
        Box::pin(async move {
            let full_path = self.resolve_path(path);
            match tokio::fs::remove_file(&full_path).await {
                Ok(_) => Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(StorageError::Io(e)),
            }
        })
    }

    fn exists<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, StorageError>> + Send + 'a>>
    {
        Box::pin(async move {
            let full_path = self.resolve_path(path);
            Ok(tokio::fs::metadata(&full_path).await.is_ok())
        })
    }
}

// ── S3 / Cloudflare R2 / MinIO Backend (with AWS SigV4) ───────────────────────

#[derive(Debug, Clone)]
pub struct S3Storage {
    client: reqwest::Client,
    endpoint: String,
    bucket: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
}

impl S3Storage {
    pub fn new(
        endpoint: impl Into<String>,
        bucket: impl Into<String>,
        region: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Self {
        let ep = endpoint.into().trim_end_matches('/').to_owned();
        Self {
            client: reqwest::Client::new(),
            endpoint: ep,
            bucket: bucket.into(),
            region: region.into(),
            access_key_id: access_key_id.into(),
            secret_access_key: secret_access_key.into(),
        }
    }

    fn build_url(&self, key: &str) -> String {
        let safe_key = key.trim_start_matches('/');
        format!("{}/{}/{}", self.endpoint, self.bucket, safe_key)
    }

    /// Signs an S3 request with AWS SigV4 and applies the Authorization header.
    fn sign_request(
        &self,
        builder: reqwest::RequestBuilder,
        method: &str,
        path_uri: &str,
        payload_hash: &str,
    ) -> reqwest::RequestBuilder {
        let now = std::time::SystemTime::now();
        let datetime = format_iso8601(now);
        let date = &datetime[0..8];

        let mut req = builder
            .header("x-amz-date", &datetime)
            .header("x-amz-content-sha256", payload_hash);

        // Compute Canonical Request
        let canonical_uri = if path_uri.starts_with('/') {
            path_uri.to_owned()
        } else {
            format!("/{}", path_uri)
        };
        let canonical_headers = format!(
            "x-amz-content-sha256:{}\nx-amz-date:{}\n",
            payload_hash, datetime
        );
        let signed_headers = "x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "{}\n{}\n\n{}\n{}\n{}",
            method, canonical_uri, canonical_headers, signed_headers, payload_hash
        );

        let hashed_canonical_request = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        // String to sign
        let scope = format!("{}/{}/s3/aws4_request", date, self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            datetime, scope, hashed_canonical_request
        );

        // Derive Signing Key
        let k_secret = format!("AWS4{}", self.secret_access_key);
        let k_date = hmac_sha256(k_secret.as_bytes(), date.as_bytes());
        let k_region = hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256(&k_region, b"s3");
        let k_signing = hmac_sha256(&k_service, b"aws4_request");

        // Calculate Signature
        let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key_id, scope, signed_headers, signature
        );

        req = req.header("Authorization", authorization);
        req
    }
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn format_iso8601(time: std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Rough ISO8601 calculation without external chrono crate
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let minutes = (rem % 3600) / 60;
    let seconds = rem % 60;

    // Approximate year/month/day calculation
    let mut y = 1970;
    let mut d = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let days_in_months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 1;
    for &dim in &days_in_months {
        if d < dim {
            break;
        }
        d -= dim;
        m += 1;
    }
    let day = d + 1;
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        y, m, day, hours, minutes, seconds
    )
}

impl BlobStorage for S3Storage {
    fn put<'a>(
        &'a self,
        path: &'a str,
        data: &'a [u8],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), StorageError>> + Send + 'a>>
    {
        Box::pin(async move {
            let url = self.build_url(path);
            let safe_key = path.trim_start_matches('/');
            let path_uri = format!("/{}/{}", self.bucket, safe_key);
            let payload_hash = hex::encode(Sha256::digest(data));

            let req_builder = self
                .client
                .put(&url)
                .header("Content-Type", "application/octet-stream")
                .body(data.to_vec());

            let signed_req = self.sign_request(req_builder, "PUT", &path_uri, &payload_hash);
            let resp = signed_req
                .send()
                .await
                .map_err(|e| StorageError::Http(e.to_string()))?;

            if resp.status().is_success() {
                Ok(())
            } else {
                Err(StorageError::Http(format!(
                    "S3 PUT failed with status: {}",
                    resp.status()
                )))
            }
        })
    }

    fn get<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<Vec<u8>>, StorageError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let url = self.build_url(path);
            let safe_key = path.trim_start_matches('/');
            let path_uri = format!("/{}/{}", self.bucket, safe_key);
            let empty_hash = hex::encode(Sha256::digest(b""));

            let req_builder = self.client.get(&url);
            let signed_req = self.sign_request(req_builder, "GET", &path_uri, &empty_hash);

            let resp = signed_req
                .send()
                .await
                .map_err(|e| StorageError::Http(e.to_string()))?;

            if resp.status() == reqwest::StatusCode::NOT_FOUND {
                Ok(None)
            } else if resp.status().is_success() {
                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| StorageError::Http(e.to_string()))?;
                Ok(Some(bytes.to_vec()))
            } else {
                Err(StorageError::Http(format!(
                    "S3 GET failed with status: {}",
                    resp.status()
                )))
            }
        })
    }

    fn delete<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, StorageError>> + Send + 'a>>
    {
        Box::pin(async move {
            let url = self.build_url(path);
            let safe_key = path.trim_start_matches('/');
            let path_uri = format!("/{}/{}", self.bucket, safe_key);
            let empty_hash = hex::encode(Sha256::digest(b""));

            let req_builder = self.client.delete(&url);
            let signed_req = self.sign_request(req_builder, "DELETE", &path_uri, &empty_hash);

            let resp = signed_req
                .send()
                .await
                .map_err(|e| StorageError::Http(e.to_string()))?;

            Ok(resp.status().is_success())
        })
    }

    fn exists<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, StorageError>> + Send + 'a>>
    {
        Box::pin(async move {
            let url = self.build_url(path);
            let safe_key = path.trim_start_matches('/');
            let path_uri = format!("/{}/{}", self.bucket, safe_key);
            let empty_hash = hex::encode(Sha256::digest(b""));

            let req_builder = self.client.head(&url);
            let signed_req = self.sign_request(req_builder, "HEAD", &path_uri, &empty_hash);

            let resp = signed_req
                .send()
                .await
                .map_err(|e| StorageError::Http(e.to_string()))?;

            Ok(resp.status().is_success())
        })
    }
}

// ── Storage Manager with Auto-Detection ───────────────────────────────────────

/// High-level Storage Manager that automatically chooses S3/R2 when environment
/// variables are configured, or defaults to durable local disk storage.
#[derive(Debug, Clone)]
pub struct StorageManager {
    backend: Arc<dyn BlobStorage>,
}

impl StorageManager {
    /// Constructs a storage manager from environment variables:
    /// - If `S3_BUCKET` or `R2_BUCKET` is present, boots an [`S3Storage`] client.
    /// - Otherwise, boots a [`LocalStorage`] client backed by `./data/storage`
    ///   (or overridden with `STORAGE_DIR`).
    pub fn from_env() -> Self {
        let bucket = std::env::var("S3_BUCKET")
            .or_else(|_| std::env::var("R2_BUCKET"))
            .unwrap_or_default();

        if !bucket.is_empty() {
            let endpoint = std::env::var("S3_ENDPOINT")
                .or_else(|_| std::env::var("R2_ENDPOINT"))
                .unwrap_or_else(|_| "https://s3.amazonaws.com".to_owned());
            let region = std::env::var("S3_REGION")
                .or_else(|_| std::env::var("R2_REGION"))
                .unwrap_or_else(|_| "auto".to_owned());
            let access_key = std::env::var("S3_ACCESS_KEY_ID")
                .or_else(|_| std::env::var("R2_ACCESS_KEY_ID"))
                .unwrap_or_default();
            let secret_key = std::env::var("S3_SECRET_ACCESS_KEY")
                .or_else(|_| std::env::var("R2_SECRET_ACCESS_KEY"))
                .unwrap_or_default();

            Self {
                backend: Arc::new(S3Storage::new(
                    endpoint, bucket, region, access_key, secret_key,
                )),
            }
        } else {
            let local_dir =
                std::env::var("STORAGE_DIR").unwrap_or_else(|_| "./data/storage".to_owned());
            Self {
                backend: Arc::new(LocalStorage::new(local_dir)),
            }
        }
    }

    /// Creates a local filesystem storage manager pointing to the specified directory.
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self {
            backend: Arc::new(LocalStorage::new(path)),
        }
    }

    /// Stores a blob/file at the given path.
    pub async fn put(&self, path: &str, data: &[u8]) -> Result<(), StorageError> {
        self.backend.put(path, data).await
    }

    /// Retrieves a blob/file by path.
    pub async fn get(&self, path: &str) -> Result<Option<Vec<u8>>, StorageError> {
        self.backend.get(path).await
    }

    /// Deletes a blob/file by path.
    pub async fn delete(&self, path: &str) -> Result<bool, StorageError> {
        self.backend.delete(path).await
    }

    /// Checks if a blob/file exists.
    pub async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        self.backend.exists(path).await
    }
}
