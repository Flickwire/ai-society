//! SQLite persistence for authoritative world checkpoints.

use std::path::Path;

use society_sim::{Village, VillageSnapshot};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

/// SQLite-backed store with a single authoritative writer connection.
#[derive(Clone, Debug)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Opens a database, enabling WAL and creating the initial schema if needed.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        migrate(&pool).await?;
        Ok(Self { pool })
    }

    /// Saves a complete validated snapshot in a durable SQLite transaction.
    pub async fn save(&self, village: &Village) -> Result<(), StorageError> {
        let snapshot = village.snapshot();
        let payload = serde_json::to_string(&snapshot)?;
        let tick = i64::try_from(village.now().0).map_err(|_| StorageError::IntegerRange)?;
        let version = i64::try_from(village.version().0).map_err(|_| StorageError::IntegerRange)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO world_snapshots (format_version, virtual_tick, world_version, payload_json) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(i64::from(snapshot.format_version))
        .bind(tick)
        .bind(version)
        .bind(payload)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Loads and validates the newest checkpoint, if one exists.
    pub async fn load(&self) -> Result<Option<Village>, StorageError> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload_json FROM world_snapshots ORDER BY sequence DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        payload
            .map(|payload| {
                let snapshot: VillageSnapshot = serde_json::from_str(&payload)?;
                Village::from_snapshot(snapshot).map_err(StorageError::InvalidSnapshot)
            })
            .transpose()
    }
}

async fn migrate(pool: &SqlitePool) -> Result<(), StorageError> {
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(include_str!("../../../migrations/0001_foundation.sql"))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Persistence failures.
#[derive(Debug)]
pub enum StorageError {
    /// SQLx reported a database or connection error.
    Sqlx(sqlx::Error),
    /// Snapshot JSON could not be encoded or decoded.
    Json(serde_json::Error),
    /// Snapshot passed JSON decoding but violated simulation invariants.
    InvalidSnapshot(society_sim::SnapshotError),
    /// A virtual-time or version value exceeded SQLite's signed integer range.
    IntegerRange,
}

impl From<sqlx::Error> for StorageError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(error) => write!(formatter, "database error: {error}"),
            Self::Json(error) => write!(formatter, "snapshot encoding error: {error}"),
            Self::InvalidSnapshot(error) => write!(formatter, "invalid world snapshot: {error:?}"),
            Self::IntegerRange => {
                formatter.write_str("world version or virtual time exceeds SQLite integer range")
            }
        }
    }
}

impl std::error::Error for StorageError {}
