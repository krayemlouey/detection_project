mod database;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use database::Database;

#[derive(Debug, Deserialize)]
struct DetectionRequestPayload {
    g_id: String,
    image_data: String, // Base64 encoded image
}

#[derive(Debug, Serialize)]
struct DetectionResponse {
    request_id: String,
    status: String,
    message: String,
    detected_objects: Option<Vec<DetectedObject>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DetectedObject {
    class: String,
    confidence: f32,
    bbox: Option<Vec<f32>>, // [x, y, width, height]
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    limit: Option<i64>,
}

// Endpoint pour effectuer une détection d'objets
async fn detect_objects(
    State(db): State<Database>,
    Json(payload): Json<DetectionRequestPayload>,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    
    // Sauvegarder la requête dans la base de données
    if let Err(e) = db.insert_detection_request(
        &payload.g_id,
        &request_id,
        &payload.image_data,
    ).await {
        tracing::error!("Erreur lors de l'insertion de la requête: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Erreur lors de la sauvegarde de la requête"}))
        ).into_response();
    }

    // Simuler la détection d'objets (remplacez par votre logique de détection)
    let detected_objects = simulate_object_detection(&payload.image_data).await;
    
    match detected_objects {
        Ok(objects) => {
            // Convertir les objets détectés en JSON pour la sauvegarde
            let objects_json = serde_json::to_string(&objects).unwrap_or_default();
            let confidence_scores = objects.iter()
                .map(|obj| obj.confidence.to_string())
                .collect::<Vec<String>>()
                .join(",");

            // Sauvegarder les résultats dans la base de données
            if let Err(e) = db.insert_detection(
                &request_id,
                &payload.g_id,
                &objects_json,
                &confidence_scores,
            ).await {
                tracing::error!("Erreur lors de l'insertion des résultats: {}", e);
            }

            Json(DetectionResponse {
                request_id,
                status: "success".to_string(),
                message: format!("{} objets détectés", objects.len()),
                detected_objects: Some(objects),
            }).into_response()
        }
        Err(e) => {
            tracing::error!("Erreur lors de la détection: {}", e);
            Json(DetectionResponse {
                request_id,
                status: "error".to_string(),
                message: "Erreur lors de la détection d'objets".to_string(),
                detected_objects: None,
            }).into_response()
        }
    }
}

// Simuler la détection d'objets (remplacez par votre modèle de détection)
async fn simulate_object_detection(image_data: &str) -> Result<Vec<DetectedObject>, Box<dyn std::error::Error>> {
    // Décoder l'image base64 avec la nouvelle API
    use base64::prelude::*;
    let _image_bytes = BASE64_STANDARD.decode(image_data)?;
    
    // Simuler des objets détectés
    let objects = vec![
        DetectedObject {
            class: "person".to_string(),
            confidence: 0.95,
            bbox: Some(vec![100.0, 50.0, 200.0, 300.0]),
        },
        DetectedObject {
            class: "car".to_string(),
            confidence: 0.87,
            bbox: Some(vec![300.0, 200.0, 150.0, 100.0]),
        },
    ];

    Ok(objects)
}

// Endpoint pour récupérer l'historique des détections
async fn get_detection_history(
    Path(g_id): Path<String>,
    Query(query): Query<HistoryQuery>,
    State(db): State<Database>,
) -> Response {
    match db.get_detections_by_gid(&g_id, query.limit).await {
        Ok(detections) => Json(json!({
            "g_id": g_id,
            "detections": detections
        })).into_response(),
        Err(e) => {
            tracing::error!("Erreur lors de la récupération de l'historique: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Erreur lors de la récupération de l'historique"}))
            ).into_response()
        }
    }
}

// Endpoint pour récupérer les statistiques
async fn get_stats(
    Path(g_id): Path<String>,
    State(db): State<Database>,
) -> Response {
    match db.get_detection_stats(&g_id).await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => {
            tracing::error!("Erreur lors de la récupération des statistiques: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Erreur lors de la récupération des statistiques"}))
            ).into_response()
        }
    }
}

// Endpoint de santé
async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "message": "Detection Backend API is running"
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialiser les logs
    tracing_subscriber::fmt::init();

    // Initialiser la base de données
    let pool = database::create_database().await?;
    let db = Database::new(pool);

    tracing::info!("🚀 Serveur de détection démarré sur http://127.0.0.1:8080");

    // Créer l'application Axum
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/detect", post(detect_objects))
        .route("/history/:g_id", get(get_detection_history))
        .route("/stats/:g_id", get(get_stats))
        .layer(CorsLayer::permissive())
        .with_state(db);

    // Démarrer le serveur
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    axum::serve(listener, app).await?;

    Ok(())
}