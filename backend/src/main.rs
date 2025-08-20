use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, Json},
    routing::{get, post, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

mod auth;
mod database;

use database::{Database, DetectionRequest, Detection, DetectionStats};

#[derive(Clone)]
struct AppState {
    db: Arc<Database>,
}

#[derive(Deserialize)]
struct DateRangeQuery {
    start_date: Option<String>,
    end_date: Option<String>,
}

#[derive(Deserialize)]
struct FilterQuery {
    color: Option<String>,
    object_type: Option<String>,
}

#[derive(Deserialize)]
struct StatsQuery {
    period: Option<String>,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: String,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "Success".to_string(),
        }
    }

    fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            message: message.to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialiser les logs
    env_logger::init();

    // Initialiser la base de données
    let db = Arc::new(Database::new().expect("Failed to initialize database"));

    let app_state = AppState { db };

    // Configuration CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Routes de l'API
    let api_routes = Router::new()
        .route("/detections", post(add_detection))
        .route("/detections", get(get_detections))
        .route("/detections/stats", get(get_detection_stats))
        .route("/detections/export", get(export_detections))
        .route("/detections/:id", delete(delete_detection))
        .route("/detections/clear", delete(clear_detections))
        .route("/auth/login", post(auth::login))
        .route("/auth/verify", post(auth::verify_token))
        .route("/health", get(health_check));

    // Routes principales
    let app = Router::new()
        .nest("/api", api_routes)
        .route("/", get(serve_index))
        .route("/login", get(serve_login))
        .route("/history", get(serve_history))
        .fallback_service(ServeDir::new("frontend"))
        .layer(ServiceBuilder::new().layer(cors))
        .with_state(app_state);

    // Démarrer le serveur
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("🚀 Serveur de détection démarré sur http://localhost:3000");
    println!("📊 Dashboard: http://localhost:3000");
    println!("🔐 Connexion: http://localhost:3000/login");
    println!("📋 Historique: http://localhost:3000/history");
    
    axum::serve(listener, app).await?;

    Ok(())
}

// Route pour servir la page d'accueil
async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../../frontend/index.html"))
}

// Route pour servir la page de connexion
async fn serve_login() -> Html<&'static str> {
    Html(include_str!("../../frontend/login.html"))
}

// Route pour servir la page d'historique
async fn serve_history() -> Html<&'static str> {
    Html(include_str!("../../frontend/history.html"))
}

// Route de vérification de santé
async fn health_check() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse::success("Service is running"))
}

// Ajouter une détection
async fn add_detection(
    State(state): State<AppState>,
    Json(request): Json<DetectionRequest>,
) -> Result<Json<ApiResponse<Detection>>, (StatusCode, Json<ApiResponse<Detection>>)> {
    println!("🔍 Nouvelle détection: {} - {} - {}", request.g_id, request.object_type, request.color);

    match state.db.add_detection(&request) {
        Ok(detection) => {
            println!("✅ Détection ajoutée: ID={}, Ref={}", 
                detection.g_id, detection.ref_count);
            Ok(Json(ApiResponse::success(detection)))
        }
        Err(e) => {
            eprintln!("❌ Erreur lors de l'ajout: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(&format!("Database error: {}", e))),
            ))
        }
    }
}

// Récupérer les détections
async fn get_detections(
    State(state): State<AppState>,
    Query(date_query): Query<DateRangeQuery>,
    Query(filter_query): Query<FilterQuery>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<Detection>>>, (StatusCode, Json<ApiResponse<Vec<Detection>>>)> {
    
    // Vérifier l'authentification pour les requêtes d'historique
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if !auth::verify_jwt_token(token) {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(ApiResponse::error("Invalid or expired token")),
                    ));
                }
            }
        }
    }

    let detections = if let (Some(start), Some(end)) = (date_query.start_date, date_query.end_date) {
        state.db.get_detections_by_date_range(&start, &end)
    } else if filter_query.color.is_some() || filter_query.object_type.is_some() {
        state.db.get_detections_by_filter(
            filter_query.color.as_deref(),
            filter_query.object_type.as_deref(),
        )
    } else {
        state.db.get_all_detections()
    };

    match detections {
        Ok(detections) => {
            println!("📊 Récupération de {} détections", detections.len());
            Ok(Json(ApiResponse::success(detections)))
        }
        Err(e) => {
            eprintln!("❌ Erreur lors de la récupération: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(&format!("Database error: {}", e))),
            ))
        }
    }
}

// Récupérer les statistiques
async fn get_detection_stats(
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<ApiResponse<DetectionStats>>, (StatusCode, Json<ApiResponse<DetectionStats>>)> {
    match state.db.get_stats() {
        Ok(stats) => {
            println!("📈 Stats générées: {} total, {} aujourd'hui", 
                stats.total_detections, stats.daily_count);
            Ok(Json(ApiResponse::success(stats)))
        }
        Err(e) => {
            eprintln!("❌ Erreur lors du calcul des stats: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(&format!("Database error: {}", e))),
            ))
        }
    }
}

// Exporter les détections
async fn export_detections(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<String, (StatusCode, Json<ApiResponse<String>>)> {
    // Vérification d'authentification
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if !auth::verify_jwt_token(token) {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(ApiResponse::error("Invalid or expired token")),
                    ));
                }
            }
        }
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Authorization token required")),
        ));
    }

    match state.db.export_detections_csv() {
        Ok(csv_content) => {
            println!("📁 Export CSV généré avec succès");
            Ok(csv_content)
        }
        Err(e) => {
            eprintln!("❌ Erreur lors de l'export: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(&format!("Export error: {}", e))),
            ))
        }
    }
}

// Supprimer une détection
async fn delete_detection(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<bool>>, (StatusCode, Json<ApiResponse<bool>>)> {
    // Vérification d'authentification
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if !auth::verify_jwt_token(token) {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(ApiResponse::error("Invalid or expired token")),
                    ));
                }
            }
        }
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Authorization token required")),
        ));
    }

    match state.db.delete_detection(id) {
        Ok(deleted) => {
            if deleted {
                println!("🗑️ Détection {} supprimée", id);
                Ok(Json(ApiResponse::success(true)))
            } else {
                Ok(Json(ApiResponse::error("Detection not found")))
            }
        }
        Err(e) => {
            eprintln!("❌ Erreur lors de la suppression: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(&format!("Database error: {}", e))),
            ))
        }
    }
}

// Vider toutes les détections
async fn clear_detections(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<&'static str>>, (StatusCode, Json<ApiResponse<&'static str>>)> {
    // Vérification d'authentification
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if !auth::verify_jwt_token(token) {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(ApiResponse::error("Invalid or expired token")),
                    ));
                }
            }
        }
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Authorization token required")),
        ));
    }

    match state.db.clear_all_detections() {
        Ok(_) => {
            println!("🧹 Toutes les détections ont été supprimées");
            Ok(Json(ApiResponse::success("All detections cleared")))
        }
        Err(e) => {
            eprintln!("❌ Erreur lors du vidage: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(&format!("Database error: {}", e))),
            ))
        }
    }
}