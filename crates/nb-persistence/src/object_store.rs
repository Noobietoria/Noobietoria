/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `ObjectStore`: durable named binary object storage.
//!
//! Used for `.nbw` world files, place assets, screenshots and user uploads.
//!
//! Implementations:
//! * [`MemoryObjectStore`] — in-memory, for tests.
//! * [`LocalFileObjectStore`] — files under a directory on disk.
//! * [`S3ObjectStore`] — S3 / Cloudflare R2 / MinIO / any S3-compatible
//!   endpoint (feature `s3`).

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::RwLock as TokioRwLock;

/// Result type for object store operations.
pub type ObjResult<T> = Result<T, ObjError>;

/// Errors an object store can raise.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ObjError {
    #[error("object not found: {0}")]
    NotFound(String),
    #[error("backend error: {0}")]
    Backend(String),
}

/// Configuration for [`S3ObjectStore`].
#[derive(Debug, Clone)]
pub struct S3ObjectStoreConfig {
    /// Endpoint URL, e.g. `https://s3.amazonaws.com`,
    /// `https://<account>.r2.cloudflarestorage.com`,
    /// `http://localhost:9000` (MinIO).
    pub endpoint: String,
    /// Bucket name.
    pub bucket: String,
    /// Access key ID.
    pub access_key: String,
    /// Secret access key.
    pub secret_key: String,
    /// Whether to use path-style URLs (needed for MinIO/R2 sometimes).
    pub path_style: bool,
}

/// Trait for async named binary object storage.
#[async_trait]
pub trait ObjectStore: Send + Sync {
    /// Store `data` under `key`, overwriting if present.
    async fn put_object(&self, key: &str, data: Bytes) -> ObjResult<()>;

    /// Retrieve the object under `key`.
    async fn get_object(&self, key: &str) -> ObjResult<Bytes>;

    /// Delete `key`.  Returns `true` if something was deleted.
    async fn delete_object(&self, key: &str) -> ObjResult<bool>;

    /// List keys matching an optional prefix.
    async fn list_objects(&self, prefix: Option<&str>) -> ObjResult<Vec<String>>;

    /// Whether `key` exists.
    async fn contains(&self, key: &str) -> ObjResult<bool>;
}

// ════════════════════════════════════════════════════════════════════════════
// In-memory implementation
// ════════════════════════════════════════════════════════════════════════════

/// An in-memory `ObjectStore`.  Nothing survives a process restart.
#[derive(Clone, Debug, Default)]
pub struct MemoryObjectStore {
    inner: Arc<TokioRwLock<std::collections::BTreeMap<String, Bytes>>>,
}

impl MemoryObjectStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ObjectStore for MemoryObjectStore {
    async fn put_object(&self, key: &str, data: Bytes) -> ObjResult<()> {
        let mut map = self.inner.write().await;
        map.insert(key.to_owned(), data);
        Ok(())
    }

    async fn get_object(&self, key: &str) -> ObjResult<Bytes> {
        let map = self.inner.read().await;
        map.get(key)
            .cloned()
            .ok_or_else(|| ObjError::NotFound(key.to_owned()))
    }

    async fn delete_object(&self, key: &str) -> ObjResult<bool> {
        let mut map = self.inner.write().await;
        Ok(map.remove(key).is_some())
    }

    async fn list_objects(&self, prefix: Option<&str>) -> ObjResult<Vec<String>> {
        let map = self.inner.read().await;
        Ok(map
            .keys()
            .filter(|k| prefix.map_or(true, |p| k.starts_with(p)))
            .cloned()
            .collect())
    }

    async fn contains(&self, key: &str) -> ObjResult<bool> {
        let map = self.inner.read().await;
        Ok(map.contains_key(key))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Local filesystem implementation
// ════════════════════════════════════════════════════════════════════════════

/// Stores objects as files under a root directory.  Keys are mapped to file
/// paths directly (sub-directories are created as needed).
#[derive(Clone, Debug)]
pub struct LocalFileObjectStore {
    root: PathBuf,
    inner: Arc<std::sync::Mutex<()>>, // serialise mutations to avoid clobbering
}

impl LocalFileObjectStore {
    /// Creates (if missing) `root` and returns a store rooted there.
    pub fn new(root: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            inner: Arc::new(std::sync::Mutex::new(())),
        })
    }

    fn path_for(&self, key: &str) -> PathBuf {
        // Sanitise key so it cannot escape the root via `..`.
        let safe = key.replace("..", "__dotdot__");
        self.root.join(safe)
    }
}

#[async_trait]
impl ObjectStore for LocalFileObjectStore {
    async fn put_object(&self, key: &str, data: Bytes) -> ObjResult<()> {
        let path = self.path_for(key);
        if let Some(parent) = path.parent() {
            let _guard = self.inner.lock().unwrap();
            std::fs::create_dir_all(parent).map_err(|e| ObjError::Backend(e.to_string()))?;
        }
        tokio::fs::write(&path, &data)
            .await
            .map_err(|e| ObjError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn get_object(&self, key: &str) -> ObjResult<Bytes> {
        let path = self.path_for(key);
        let data = tokio::fs::read(&path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ObjError::NotFound(key.to_owned()),
            _ => ObjError::Backend(e.to_string()),
        })?;
        Ok(Bytes::from(data))
    }

    async fn delete_object(&self, key: &str) -> ObjResult<bool> {
        let path = self.path_for(key);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(ObjError::Backend(e.to_string())),
        }
    }

    async fn list_objects(&self, prefix: Option<&str>) -> ObjResult<Vec<String>> {
        let mut keys = Vec::new();
        let _guard = self.inner.lock().unwrap();
        Self::walk_dir(&self.root, prefix, &self.root, &mut keys)
            .map_err(|e| ObjError::Backend(e.to_string()))?;
        keys.sort();
        Ok(keys)
    }

    async fn contains(&self, key: &str) -> ObjResult<bool> {
        let path = self.path_for(key);
        Ok(tokio::fs::try_exists(&path).await.unwrap_or(false))
    }
}

impl LocalFileObjectStore {
    fn walk_dir(
        dir: &PathBuf,
        prefix: Option<&str>,
        root: &PathBuf,
        keys: &mut Vec<String>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::walk_dir(&path, prefix, root, keys)?;
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if prefix.map_or(true, |p| rel.starts_with(p)) {
                    keys.push(rel);
                }
            }
        }
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// S3 / R2 implementation
// ════════════════════════════════════════════════════════════════════════════

/// S3-compatible object store (AWS S3, Cloudflare R2, MinIO, etc.).
///
/// Requires the `s3` feature of this crate.
#[cfg(feature = "s3")]
#[derive(Clone, Debug)]
pub struct S3ObjectStore {
    client: s3::Client,
    bucket: String,
}

#[cfg(feature = "s3")]
impl S3ObjectStore {
    /// Connect using `config`.
    pub async fn new(config: S3ObjectStoreConfig) -> anyhow::Result<Self> {
        use std::sync::Arc;

        let region = s3::Region::Custom {
            region: "auto".to_owned(),
            endpoint: config.endpoint.clone(),
        };

        let conf = s3::client::ClientConfig::new();
        let client = s3::Client::from_conf(
            conf.with_region(region).with_credentials_provider(Arc::new(
                s3::creds::Credentials::new(
                    Some(&config.access_key),
                    Some(&config.secret_key),
                    None,
                    None,
                    "static",
                )
                .map_err(|e| anyhow::anyhow!("bad credentials: {e}"))?,
            )),
        );

        Ok(Self {
            client,
            bucket: config.bucket,
        })
    }
}

#[cfg(feature = "s3")]
#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put_object(&self, key: &str, data: Bytes) -> ObjResult<()> {
        let _ = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(data.into())
            .send()
            .await
            .map_err(|e| ObjError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn get_object(&self, key: &str) -> ObjResult<Bytes> {
        let out = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| match e.to_string().as_str() {
                s if s.contains("NoSuchKey") => ObjError::NotFound(key.to_owned()),
                _ => ObjError::Backend(e.to_string()),
            })?;
        let data = out
            .body
            .collect()
            .await
            .map_err(|e| ObjError::Backend(e.to_string()))?;
        Ok(data.into_bytes().into())
    }

    async fn delete_object(&self, key: &str) -> ObjResult<bool> {
        let _ = self
            .client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| ObjError::Backend(e.to_string()))?;
        Ok(true)
    }

    async fn list_objects(&self, prefix: Option<&str>) -> ObjResult<Vec<String>> {
        let mut keys = Vec::new();
        let mut continuation_token: Option<String> = None;
        loop {
            let mut req = self.client.list_objects_v2().bucket(&self.bucket);
            if let Some(p) = prefix {
                req = req.prefix(p);
            }
            if let Some(t) = &continuation_token {
                req = req.continuation_token(t);
            }
            let out = req
                .send()
                .await
                .map_err(|e| ObjError::Backend(e.to_string()))?;
            for obj in out.contents().unwrap_or_default() {
                if let Some(k) = obj.key() {
                    keys.push(k.to_owned());
                }
            }
            continuation_token = out.next_continuation_token().map(|s| s.to_owned());
            if continuation_token.is_none() {
                break;
            }
        }
        keys.sort();
        Ok(keys)
    }

    async fn contains(&self, key: &str) -> ObjResult<bool> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) if e.to_string().contains("NotFound") => Ok(false),
            Err(e) => Err(ObjError::Backend(e.to_string())),
        }
    }
}

#[cfg(not(feature = "s3"))]
/// Placeholder when the `s3` feature is disabled.
#[derive(Clone, Debug)]
pub struct S3ObjectStore;

#[cfg(not(feature = "s3"))]
impl S3ObjectStore {
    pub async fn new(_config: S3ObjectStoreConfig) -> anyhow::Result<Self> {
        Err(anyhow::anyhow!(
            "the `s3` feature of nb-persistence is not enabled"
        ))
    }
}

#[cfg(not(feature = "s3"))]
#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put_object(&self, _key: &str, _data: Bytes) -> ObjResult<()> {
        Err(ObjError::Backend("s3 feature disabled".into()))
    }
    async fn get_object(&self, _key: &str) -> ObjResult<Bytes> {
        Err(ObjError::Backend("s3 feature disabled".into()))
    }
    async fn delete_object(&self, _key: &str) -> ObjResult<bool> {
        Err(ObjError::Backend("s3 feature disabled".into()))
    }
    async fn list_objects(&self, _prefix: Option<&str>) -> ObjResult<Vec<String>> {
        Err(ObjError::Backend("s3 feature disabled".into()))
    }
    async fn contains(&self, _key: &str) -> ObjResult<bool> {
        Err(ObjError::Backend("s3 feature disabled".into()))
    }
}
