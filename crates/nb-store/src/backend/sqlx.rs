/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Sqlx-based backend supporting Postgres, MySQL, CockroachDB, and SQLite.

use std::collections::BTreeMap;
use std::sync::Arc;

use nb_types::Value;
use sqlx::{AnyPool, Row};

use crate::backend::KvBackend;
use crate::kv::StoreError;

/// Real persistent storage backed by PostgreSQL, MySQL, CockroachDB, or SQLite.
#[derive(Clone, Debug)]
pub struct SqlxBackend {
    table_name: String,
    pool: AnyPool,
    rt: Arc<parking_lot::Mutex<Option<tokio::runtime::Runtime>>>,
}

impl SqlxBackend {
    /// Connects to any supported SQL database (Postgres, CockroachDB, MySQL, SQLite)
    /// asynchronously, creating the storage table if needed.
    pub async fn connect_async(database_url: &str, table_name: &str) -> Result<Self, String> {
        sqlx::any::install_default_drivers();
        let pool = AnyPool::connect(database_url)
            .await
            .map_err(|e| format!("Failed to connect to database `{database_url}`: {e}"))?;

        let safe_table = sanitize_table_name(table_name);
        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {safe_table} (\
                kv_key VARCHAR(255) PRIMARY KEY, \
                kv_value TEXT NOT NULL\
            )"
        );

        sqlx::query(&create_sql)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to create table `{safe_table}`: {e}"))?;

        Ok(Self {
            table_name: safe_table,
            pool,
            rt: Arc::new(parking_lot::Mutex::new(None)),
        })
    }

    /// Synchronous constructor for environments without an existing async runtime.
    pub fn connect(database_url: &str, table_name: &str) -> Result<Self, String> {
        // Run on a dedicated thread with its own runtime so we never nest
        // runtimes (which panics when called from within `#[tokio::main]`).
        let url = database_url.to_owned();
        let table = table_name.to_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to start runtime: {e}")));
                    return;
                }
            };
            let result = rt.block_on(async move {
                sqlx::any::install_default_drivers();
                let pool = AnyPool::connect(&url)
                    .await
                    .map_err(|e| format!("Failed to connect to `{url}`: {e}"))?;
                let safe_table = sanitize_table_name(&table);
                let create_sql = format!(
                    "CREATE TABLE IF NOT EXISTS {safe_table} (\
                        kv_key VARCHAR(255) PRIMARY KEY, \
                        kv_value TEXT NOT NULL\
                    )"
                );
                sqlx::query(&create_sql)
                    .execute(&pool)
                    .await
                    .map_err(|e| format!("Failed to create table `{safe_table}`: {e}"))?;
                Ok::<_, String>((safe_table, pool))
            });
            match result {
                Ok((safe_table, pool)) => {
                    let _ = tx.send(Ok(Self {
                        table_name: safe_table,
                        pool,
                        // Leave rt empty — block_on() creates one on demand
                        // when no external runtime is available.
                        rt: Arc::new(parking_lot::Mutex::new(None)),
                    }));
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            }
        });
        rx.recv()
            .map_err(|_| "SqlxBackend connection thread panicked".to_owned())?
    }

    /// Returns a reference to the underlying [`AnyPool`].
    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }

    fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(f))
        } else {
            let mut guard = self.rt.lock();
            if guard.is_none() {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build tokio runtime");
                *guard = Some(rt);
            }
            guard.as_mut().unwrap().block_on(f)
        }
    }
}

fn sanitize_table_name(raw: &str) -> String {
    let filtered: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if filtered.is_empty() {
        "nb_kv_store".to_owned()
    } else {
        filtered
    }
}

impl KvBackend for SqlxBackend {
    fn get(&self, key: &str) -> Result<Option<Value>, StoreError> {
        let sql = format!("SELECT kv_value FROM {} WHERE kv_key = $1", self.table_name);
        let pool = self.pool.clone();
        let key_str = key.to_owned();

        let row_opt = self
            .block_on(async move { sqlx::query(&sql).bind(key_str).fetch_optional(&pool).await });

        match row_opt {
            Ok(Some(row)) => {
                let raw_json: String = row.get(0);
                let val: Value =
                    serde_json::from_str(&raw_json).map_err(|_| StoreError::NotAString {
                        key: key.to_owned(),
                        actual: "corrupted_json",
                    })?;
                Ok(Some(val))
            }
            Ok(None) => Ok(None),
            Err(_e) => Err(StoreError::NotAString {
                key: key.to_owned(),
                actual: "sqlx_error",
            }),
        }
    }

    fn set(&self, key: &str, value: &Value) -> Result<(), StoreError> {
        let serialized = serde_json::to_string(value).map_err(|_| StoreError::NotAString {
            key: key.to_owned(),
            actual: "serialization_error",
        })?;

        let del_sql = format!("DELETE FROM {} WHERE kv_key = $1", self.table_name);
        let ins_sql = format!(
            "INSERT INTO {} (kv_key, kv_value) VALUES ($1, $2)",
            self.table_name
        );
        let pool = self.pool.clone();
        let k1 = key.to_owned();
        let k2 = key.to_owned();

        self.block_on(async move {
            let _ = sqlx::query(&del_sql).bind(k1).execute(&pool).await;
            sqlx::query(&ins_sql)
                .bind(k2)
                .bind(serialized)
                .execute(&pool)
                .await
        })
        .map_err(|_| StoreError::NotAString {
            key: key.to_owned(),
            actual: "sqlx_insert_error",
        })?;

        Ok(())
    }

    fn remove(&self, key: &str) -> Result<Option<Value>, StoreError> {
        let prev = self.get(key)?;
        if prev.is_some() {
            let del_sql = format!("DELETE FROM {} WHERE kv_key = $1", self.table_name);
            let pool = self.pool.clone();
            let k = key.to_owned();
            let _ =
                self.block_on(async move { sqlx::query(&del_sql).bind(k).execute(&pool).await });
        }
        Ok(prev)
    }

    fn contains(&self, key: &str) -> Result<bool, StoreError> {
        Ok(self.get(key)?.is_some())
    }

    fn keys(&self) -> Result<Vec<String>, StoreError> {
        let sql = format!("SELECT kv_key FROM {} ORDER BY kv_key ASC", self.table_name);
        let pool = self.pool.clone();

        let rows = self
            .block_on(async move { sqlx::query(&sql).fetch_all(&pool).await })
            .map_err(|_| StoreError::NotAString {
                key: "*".to_owned(),
                actual: "sqlx_keys_error",
            })?;

        let keys = rows.into_iter().map(|r| r.get(0)).collect();
        Ok(keys)
    }

    fn clear(&self) -> Result<(), StoreError> {
        let sql = format!("DELETE FROM {}", self.table_name);
        let pool = self.pool.clone();
        self.block_on(async move { sqlx::query(&sql).execute(&pool).await })
            .map_err(|_| StoreError::NotAString {
                key: "*".to_owned(),
                actual: "sqlx_clear_error",
            })?;
        Ok(())
    }

    fn snapshot(&self) -> Result<BTreeMap<String, Value>, StoreError> {
        let sql = format!(
            "SELECT kv_key, kv_value FROM {} ORDER BY kv_key ASC",
            self.table_name
        );
        let pool = self.pool.clone();

        let rows = self
            .block_on(async move { sqlx::query(&sql).fetch_all(&pool).await })
            .map_err(|_| StoreError::NotAString {
                key: "*".to_owned(),
                actual: "sqlx_snapshot_error",
            })?;

        let mut map = BTreeMap::new();
        for row in rows {
            let k: String = row.get(0);
            let v_str: String = row.get(1);
            if let Ok(val) = serde_json::from_str(&v_str) {
                map.insert(k, val);
            }
        }
        Ok(map)
    }

    fn restore(&self, map: BTreeMap<String, Value>) -> Result<(), StoreError> {
        self.clear()?;
        for (k, v) in map {
            self.set(&k, &v)?;
        }
        Ok(())
    }
}
