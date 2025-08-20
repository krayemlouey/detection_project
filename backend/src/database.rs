use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use serde_json::{json, Value};
use chrono::Local;
use std::fs;

/// Initialise la connexion à la base de données SQLite et crée les tables si nécessaire
pub async fn init_database() -> Result<SqlitePool, sqlx::Error> {
    // Créer le répertoire de base de données s'il n'existe pas
    std::fs::create_dir_all("./data").map_err(|e| {
        eprintln!("❌ Erreur création répertoire: {}", e);
        sqlx::Error::Io(e)
    })?;
    
    let database_url = "sqlite:./data/detection.db";
    
    println!("📊 Connexion à la base de données SQLite...");
    println!("📁 Chemin de la base: {}", database_url);
    
    let pool = SqlitePool::connect(database_url).await.map_err(|e| {
        eprintln!("❌ Erreur connexion base de données: {}", e);
        eprintln!("💡 Vérifiez les permissions d'écriture dans le répertoire");
        e
    })?;

    create_tables(&pool).await?;

    println!("✅ Base de données initialisée avec succès");
    Ok(pool)
}

/// Crée les tables nécessaires dans la base de données
async fn create_tables(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Table des détections individuelles
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS detections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            g_id TEXT NOT NULL,
            ref_count INTEGER NOT NULL DEFAULT 1,
            type TEXT NOT NULL,
            color TEXT NOT NULL,
            datetime TEXT NOT NULL,
            UNIQUE(g_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Table des statistiques journalières
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daily_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            g_id TEXT NOT NULL,
            type TEXT NOT NULL,
            cadence INTEGER NOT NULL DEFAULT 0,
            day TEXT NOT NULL,
            UNIQUE(g_id, day)
        )
        "#,
    )
    .execute(pool)
    .await?;

    println!("📋 Tables créées/vérifiées");
    Ok(())
}

/// Insère une nouvelle détection ou met à jour si elle existe déjà 
pub async fn insert_detection(
    pool: &SqlitePool,
    g_id: &str,
    object_type: &str,
    color: &str,
) -> Result<(), sqlx::Error> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let today = Local::now().format("%Y-%m-%d").to_string();

    let mut tx = pool.begin().await?;

    // Insert/update détection
    sqlx::query(
        r#"
        INSERT INTO detections (g_id, ref_count, type, color, datetime)
        VALUES (?, 1, ?, ?, ?)
        ON CONFLICT(g_id) DO UPDATE SET
            ref_count = ref_count + 1,
            datetime = excluded.datetime
        "#,
    )
    .bind(g_id)
    .bind(object_type)
    .bind(color)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    // Insert/update statistiques journalières
    sqlx::query(
        r#"
        INSERT INTO daily_stats (g_id, type, cadence, day)
        VALUES (?, ?, 1, ?)
        ON CONFLICT(g_id, day) DO UPDATE SET
            cadence = cadence + 1
        "#,
    )
    .bind(g_id)
    .bind(object_type)
    .bind(&today)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    println!("🔍 Détection ajoutée: {} ({}) - {}", g_id, object_type, color);
    Ok(())
}

/// Récupère les détections avec filtres optionnels sur la date
pub async fn get_detections(
    pool: &SqlitePool,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<Vec<Value>, sqlx::Error> {
    let rows = match (&from_date, &to_date) {
        (Some(from), Some(to)) => {
            sqlx::query("SELECT g_id, ref_count, type, color, datetime FROM detections WHERE date(datetime) >= ? AND date(datetime) <= ? ORDER BY datetime DESC")
                .bind(from)
                .bind(to)
                .fetch_all(pool)
                .await?
        }
        (Some(from), None) => {
            sqlx::query("SELECT g_id, ref_count, type, color, datetime FROM detections WHERE date(datetime) >= ? ORDER BY datetime DESC")
                .bind(from)
                .fetch_all(pool)
                .await?
        }
        (None, Some(to)) => {
            sqlx::query("SELECT g_id, ref_count, type, color, datetime FROM detections WHERE date(datetime) <= ? ORDER BY datetime DESC")
                .bind(to)
                .fetch_all(pool)
                .await?
        }
        (None, None) => {
            sqlx::query("SELECT g_id, ref_count, type, color, datetime FROM detections ORDER BY datetime DESC")
                .fetch_all(pool)
                .await?
        }
    };

    let detections: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "g_id": row.get::<String, _>("g_id"),
                "ref_count": row.get::<i64, _>("ref_count"),
                "type": row.get::<String, _>("type"),
                "color": row.get::<String, _>("color"),
                "datetime": row.get::<String, _>("datetime")
            })
        })
        .collect();

    Ok(detections)
}

/// Récupère les statistiques journalières et récentes
pub async fn get_daily_stats(pool: &SqlitePool) -> Result<Value, sqlx::Error> {
    let today = Local::now().format("%Y-%m-%d").to_string();

    let today_stats = sqlx::query(
        "SELECT type, SUM(cadence) as total FROM daily_stats WHERE day = ? GROUP BY type"
    )
    .bind(&today)
    .fetch_all(pool)
    .await?;

    let total_stats = sqlx::query(
        "SELECT type, SUM(cadence) as total FROM daily_stats GROUP BY type"
    )
    .fetch_all(pool)
    .await?;

    let recent_detections = sqlx::query(
        "SELECT g_id, type, color, datetime FROM detections ORDER BY datetime DESC LIMIT 10"
    )
    .fetch_all(pool)
    .await?;

    let today_data: Vec<Value> = today_stats
        .into_iter()
        .map(|row| json!({
            "type": row.get::<String, _>("type"),
            "count": row.get::<i64, _>("total")
        }))
        .collect();

    let total_data: Vec<Value> = total_stats
        .into_iter()
        .map(|row| json!({
            "type": row.get::<String, _>("type"),
            "count": row.get::<i64, _>("total")
        }))
        .collect();

    let recent_data: Vec<Value> = recent_detections
        .into_iter()
        .map(|row| json!({
            "g_id": row.get::<String, _>("g_id"),
            "type": row.get::<String, _>("type"),
            "color": row.get::<String, _>("color"),
            "datetime": row.get::<String, _>("datetime")
        }))
        .collect();

    Ok(json!({
        "today": today_data,
        "total": total_data,
        "recent": recent_data,
        "last_updated": Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }))
}

/// Supprime les anciennes statistiques (maintenance)
#[allow(dead_code)]
pub async fn cleanup_old_data(pool: &SqlitePool, days_to_keep: i64) -> Result<(), sqlx::Error> {
    let cutoff_date = Local::now()
        .checked_sub_signed(chrono::Duration::days(days_to_keep))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();

    let deleted = sqlx::query("DELETE FROM daily_stats WHERE day < ?")
        .bind(&cutoff_date)
        .execute(pool)
        .await?;

    println!("🧹 Nettoyage: {} entrées supprimées", deleted.rows_affected());
    Ok(())
}