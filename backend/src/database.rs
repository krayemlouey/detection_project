///database.rs
use sqlx::{sqlite::SqlitePool, Row};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local, NaiveDate};
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    pub id: i64,
    pub g_id: String,
    pub ref_count: i64,
    pub object_type: String,
    pub color: String,
    pub confidence: Option<f32>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStat {
    pub id: i64,
    pub object_type: String,
    pub count: i64,
    pub date: NaiveDate,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub today: HashMap<String, i64>,
    pub total: HashMap<String, i64>,
    pub recent: Vec<Detection>,
    pub daily_trend: Vec<DailyStat>,
    pub last_updated: DateTime<Local>,
}

/// Initialise la base de données avec les migrations
pub async fn init_database() -> Result<SqlitePool> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./detection.db".to_string());
    
    tracing::info!("📊 Connexion à la base de données: {}", database_url);
    
    let pool = SqlitePool::connect(&database_url).await?;
    
    // Activer les clés étrangères et optimiser SQLite
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;
        
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await?;
    
    create_tables(&pool).await?;
    create_indexes(&pool).await?;
    
    tracing::info!("✅ Base de données initialisée avec succès");
    Ok(pool)
}

/// Crée les tables avec une structure optimisée
async fn create_tables(pool: &SqlitePool) -> Result<()> {
    // Table principale des détections
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS detections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            g_id TEXT NOT NULL UNIQUE,
            ref_count INTEGER NOT NULL DEFAULT 1,
            object_type TEXT NOT NULL,
            color TEXT NOT NULL,
            confidence REAL DEFAULT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Table des statistiques journalières (agrégées)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daily_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            object_type TEXT NOT NULL,
            count INTEGER NOT NULL DEFAULT 0,
            date DATE NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(object_type, date)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Trigger pour mettre à jour updated_at automatiquement
    sqlx::query(
        r#"
        CREATE TRIGGER IF NOT EXISTS update_detections_timestamp 
        AFTER UPDATE ON detections
        BEGIN
            UPDATE detections SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
        END
        "#,
    )
    .execute(pool)
    .await?;

    tracing::info!("📋 Tables créées/vérifiées");
    Ok(())
}

/// Crée les index pour optimiser les performances
async fn create_indexes(pool: &SqlitePool) -> Result<()> {
    let indexes = vec![
        "CREATE INDEX IF NOT EXISTS idx_detections_type ON detections(object_type)",
        "CREATE INDEX IF NOT EXISTS idx_detections_created_at ON detections(created_at)",
        "CREATE INDEX IF NOT EXISTS idx_detections_updated_at ON detections(updated_at)",
        "CREATE INDEX IF NOT EXISTS idx_daily_stats_date ON daily_stats(date)",
        "CREATE INDEX IF NOT EXISTS idx_daily_stats_type_date ON daily_stats(object_type, date)",
    ];

    for index_sql in indexes {
        sqlx::query(index_sql).execute(pool).await?;
    }

    tracing::info("🔍 Index créés pour optimiser les performances");
    Ok(())
}

/// Insère ou met à jour une détection de manière optimisée
pub async fn upsert_detection(
    pool: &SqlitePool,
    g_id: &str,
    object_type: &str,
    color: &str,
    confidence: Option<f32>,
) -> Result<Detection> {
    let mut tx = pool.begin().await?;

    // Insérer ou mettre à jour la détection
    let detection = sqlx::query_as!(
        Detection,
        r#"
        INSERT INTO detections (g_id, object_type, color, confidence, ref_count)
        VALUES (?, ?, ?, ?, 1)
        ON CONFLICT(g_id) DO UPDATE SET
            ref_count = ref_count + 1,
            color = excluded.color,
            confidence = excluded.confidence,
            updated_at = CURRENT_TIMESTAMP
        RETURNING 
            id,
            g_id,
            ref_count,
            object_type,
            color,
            confidence,
            created_at as "created_at: DateTime<Local>",
            updated_at as "updated_at: DateTime<Local>"
        "#,
        g_id,
        object_type,
        color,
        confidence
    )
    .fetch_one(&mut *tx)
    .await?;

    // Mettre à jour les statistiques journalières
    let today = Local::now().date_naive();
    sqlx::query(
        r#"
        INSERT INTO daily_stats (object_type, count, date)
        VALUES (?, 1, ?)
        ON CONFLICT(object_type, date) DO UPDATE SET
            count = count + 1
        "#,
    )
    .bind(object_type)
    .bind(today)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(
        "🔍 Détection mise à jour: {} ({}) - {} [confiance: {:?}]",
        g_id, object_type, color, confidence
    );

    Ok(detection)
}

/// Récupère les détections avec pagination et filtres
pub async fn get_detections(
    pool: &SqlitePool,
    from_date: Option<String>,
    to_date: Option<String>,
    object_type: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Detection>> {
    let limit = limit.unwrap_or(100).min(1000); // Max 1000 résultats
    let offset = offset.unwrap_or(0);

    let mut query = String::from(
        r#"
        SELECT 
            id,
            g_id,
            ref_count,
            object_type,
            color,
            confidence,
            created_at,
            updated_at
        FROM detections 
        WHERE 1=1
        "#
    );
    
    let mut params: Vec<&dyn sqlx::Encode<sqlx::Sqlite> + Send + Sync> = Vec::new();

    if let Some(from) = &from_date {
        query.push_str(" AND date(created_at) >= ?");
        params.push(from);
    }
    if let Some(to) = &to_date {
        query.push_str(" AND date(created_at) <= ?");
        params.push(to);
    }
    if let Some(obj_type) = &object_type {
        query.push_str(" AND object_type = ?");
        params.push(obj_type);
    }

    query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

    let mut sql_query = sqlx::query_as::<_, Detection>(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(limit).bind(offset);

    let detections = sql_query.fetch_all(pool).await?;
    Ok(detections)
}

/// Récupère les statistiques complètes
pub async fn get_comprehensive_stats(pool: &SqlitePool) -> Result<StatsResponse> {
    let today = Local::now().date_naive();

    // Stats d'aujourd'hui
    let today_stats = sqlx::query!(
        "SELECT object_type, count FROM daily_stats WHERE date = ?",
        today
    )
    .fetch_all(pool)
    .await?;

    let today_map: HashMap<String, i64> = today_stats
        .into_iter()
        .map(|row| (row.object_type, row.count))
        .collect();

    // Stats totales
    let total_stats = sqlx::query!(
        "SELECT object_type, SUM(count) as total FROM daily_stats GROUP BY object_type"
    )
    .fetch_all(pool)
    .await?;

    let total_map: HashMap<String, i64> = total_stats
        .into_iter()
        .map(|row| (row.object_type, row.total.unwrap_or(0)))
        .collect();

    // Détections récentes
    let recent_detections = sqlx::query_as!(
        Detection,
        r#"
        SELECT 
            id,
            g_id,
            ref_count,
            object_type,
            color,
            confidence,
            created_at as "created_at: DateTime<Local>",
            updated_at as "updated_at: DateTime<Local>"
        FROM detections 
        ORDER BY updated_at DESC 
        LIMIT 10
        "#
    )
    .fetch_all(pool)
    .await?;

    // Tendance des 7 derniers jours
    let daily_trend = sqlx::query_as!(
        DailyStat,
        r#"
        SELECT 
            id,
            object_type,
            count,
            date
        FROM daily_stats 
        WHERE date >= date('now', '-7 days')
        ORDER BY date DESC, object_type
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(StatsResponse {
        today: today_map,
        total: total_map,
        recent: recent_detections,
        daily_trend,
        last_updated: Local::now(),
    })
}

/// Nettoie les anciennes données (maintenance)
pub async fn cleanup_old_data(pool: &SqlitePool, days_to_keep: i64) -> Result<u64> {
    let cutoff_date = Local::now()
        .checked_sub_signed(chrono::Duration::days(days_to_keep))
        .unwrap()
        .date_naive();

    let mut tx = pool.begin().await?;

    // Supprimer les vieilles détections
    let detections_deleted = sqlx::query!(
        "DELETE FROM detections WHERE date(created_at) < ?",
        cutoff_date
    )
    .execute(&mut *tx)
    .await?;

    // Supprimer les vieilles stats
    let stats_deleted = sqlx::query!(
        "DELETE FROM daily_stats WHERE date < ?",
        cutoff_date
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let total_deleted = detections_deleted.rows_affected() + stats_deleted.rows_affected();
    tracing::info!("🧹 Nettoyage: {} entrées supprimées", total_deleted);
    
    Ok(total_deleted)
}

/// Exporte les données en format CSV
pub async fn export_to_csv(
    pool: &SqlitePool,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<String> {
    let detections = get_detections(pool, from_date, to_date, None, None, None).await?;
    
    let mut csv_content = String::new();
    csv_content.push_str("ID,G_ID,Type,Couleur,Confiance,Occurrences,Date_Creation,Derniere_MAJ\n");
    
    for detection in detections {
        csv_content.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            detection.id,
            detection.g_id,
            detection.object_type,
            detection.color,
            detection.confidence.map_or("N/A".to_string(), |c| c.to_string()),
            detection.ref_count,
            detection.created_at.format("%Y-%m-%d %H:%M:%S"),
            detection.updated_at.format("%Y-%m-%d %H:%M:%S")
        ));
    }
    
    Ok(csv_content)
}