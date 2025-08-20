use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Fonction pour créer la base de données et les tables
pub async fn create_database() -> Result<SqlitePool, sqlx::Error> {
    // Créer le répertoire data s'il n'existe pas
    std::fs::create_dir_all("data").map_err(|e| {
        sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create data directory: {}", e)
        ))
    })?;
    
    let pool = SqlitePool::connect("sqlite:data/detection.db").await?;

    // Créer les tables si elles n'existent pas
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS detection_requests (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            g_id TEXT NOT NULL,
            request_id TEXT NOT NULL UNIQUE,
            image_data TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            status TEXT DEFAULT 'pending'
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS detections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            request_id TEXT NOT NULL,
            g_id TEXT NOT NULL,
            detected_objects TEXT NOT NULL,
            confidence_scores TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (request_id) REFERENCES detection_requests (request_id)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

// Structure pour les requêtes de détection
#[derive(Debug, Serialize, Deserialize)]
pub struct DetectionRequest {
    pub id: Option<i64>,
    pub g_id: String,
    pub request_id: String,
    pub image_data: String,
    pub timestamp: Option<String>,
    pub status: Option<String>,
}

// Structure pour les résultats de détection
#[derive(Debug, Serialize, Deserialize)]
pub struct Detection {
    pub id: Option<i64>,
    pub request_id: String,
    pub g_id: String,
    pub detected_objects: String,
    pub confidence_scores: String,
    pub timestamp: Option<String>,
}

// Structure pour les statistiques
#[derive(Debug, Serialize, Deserialize)]
pub struct DetectionStats {
    pub today_count: i64,
    pub total_count: i64,
    pub recent_detections: Vec<Value>,
}

// Structure principale pour la base de données
#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // Insérer une nouvelle requête de détection
    pub async fn insert_detection_request(
        &self,
        g_id: &str,
        request_id: &str,
        image_data: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO detection_requests (g_id, request_id, image_data) VALUES (?, ?, ?)"
        )
        .bind(g_id)
        .bind(request_id)
        .bind(image_data)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Insérer un résultat de détection
    pub async fn insert_detection(
        &self,
        request_id: &str,
        g_id: &str,
        detected_objects: &str,
        confidence_scores: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO detections (request_id, g_id, detected_objects, confidence_scores) VALUES (?, ?, ?, ?)"
        )
        .bind(request_id)
        .bind(g_id)
        .bind(detected_objects)
        .bind(confidence_scores)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Récupérer les détections par g_id
    pub async fn get_detections_by_gid(
        &self,
        g_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<Value>, sqlx::Error> {
        let limit = limit.unwrap_or(50);

        let query = "SELECT g_id, request_id, detected_objects, confidence_scores, timestamp FROM detections WHERE g_id = ? ORDER BY timestamp DESC LIMIT ?";
        let rows = sqlx::query(query)
            .bind(g_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut detections = Vec::new();
        for row in rows {
            let detection = serde_json::json!({
                "g_id": row.get::<String, _>("g_id"),
                "request_id": row.get::<String, _>("request_id"),
                "detected_objects": row.get::<String, _>("detected_objects"),
                "confidence_scores": row.get::<String, _>("confidence_scores"),
                "timestamp": row.get::<String, _>("timestamp"),
            });
            detections.push(detection);
        }

        Ok(detections)
    }

    // Récupérer les statistiques de détection
    pub async fn get_detection_stats(&self, g_id: &str) -> Result<Value, sqlx::Error> {
        // Statistiques d'aujourd'hui
        let today_stats = sqlx::query(
            "SELECT COUNT(*) as count FROM detections WHERE g_id = ? AND DATE(timestamp) = DATE('now')"
        )
        .bind(g_id)
        .fetch_one(&self.pool)
        .await?;

        // Statistiques totales
        let total_stats = sqlx::query(
            "SELECT COUNT(*) as count FROM detections WHERE g_id = ?"
        )
        .bind(g_id)
        .fetch_one(&self.pool)
        .await?;

        // Détections récentes
        let recent_detections = sqlx::query(
            "SELECT request_id, detected_objects, confidence_scores, timestamp 
             FROM detections 
             WHERE g_id = ? 
             ORDER BY timestamp DESC 
             LIMIT 5"
        )
        .bind(g_id)
        .fetch_all(&self.pool)
        .await?;

        let mut recent_vec = Vec::new();
        for row in recent_detections {
            let detection = serde_json::json!({
                "request_id": row.get::<String, _>("request_id"),
                "detected_objects": row.get::<String, _>("detected_objects"),
                "confidence_scores": row.get::<String, _>("confidence_scores"),
                "timestamp": row.get::<String, _>("timestamp"),
            });
            recent_vec.push(detection);
        }

        let stats = serde_json::json!({
            "today_count": today_stats.get::<i64, _>("count"),
            "total_count": total_stats.get::<i64, _>("count"),
            "recent_detections": recent_vec
        });

        Ok(stats)
    }

    // Supprimer les anciennes détections (plus de 30 jours)
    #[allow(dead_code)]
    pub async fn cleanup_old_detections(&self) -> Result<(), sqlx::Error> {
        // Supprimer les détections de plus de 30 jours
        let deleted = sqlx::query("DELETE FROM detections WHERE timestamp < DATE('now', '-30 days')")
            .execute(&self.pool)
            .await?;

        // Supprimer les requêtes de détection correspondantes
        sqlx::query("DELETE FROM detection_requests WHERE timestamp < DATE('now', '-30 days')")
            .execute(&self.pool)
            .await?;

        tracing::info!("Supprimé {} anciennes détections", deleted.rows_affected());
        Ok(())
    }
}