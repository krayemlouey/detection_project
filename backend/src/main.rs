use axum::{
    extract::{Query, State, Path},
    http::{HeaderValue, Method, StatusCode, HeaderMap},
    middleware,
    response::{Json, Html, IntoResponse},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    compression::CompressionLayer,
    trace::TraceLayer,
};
use std::collections::HashMap;
use anyhow::Result;
use validator::Validate;

// Modules internes
mod auth;
mod database;
mod security;

// Import du module de sécurité
use security::security_middleware;

// État partagé de l'application
#[derive(Clone)]
pub struct AppState {
    db: SqlitePool,
}

// Structures de réponse API standardisées
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: String,
    timestamp: String,
}

impl<T> ApiResponse<T> {
    fn success(data: T, message: &str) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: message.to_string(),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }

    fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            message: message.to_string(),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

// Structures de requêtes
#[derive(Deserialize, Validate)]
struct DetectionRequest {
    #[validate(length(min = 1, max = 100, message = "G_ID doit contenir entre 1 et 100 caractères"))]
    pub g_id: String,
    
    #[validate(length(min = 1, max = 50, message = "Type d'objet requis"))]
    pub object_type: String,
    
    #[validate(length(min = 1, max = 30, message = "Couleur requise"))]
    pub color: String,
    
    #[validate(range(min = 0.0, max = 1.0, message = "Confiance doit être entre 0 et 1"))]
    pub confidence: Option<f32>,
}

#[derive(Deserialize)]
struct HistoryQuery {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub object_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
struct ExportQuery {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub format: Option<String>, // "csv" ou "json"
}

// Middleware d'authentification amélioré
async fn auth_middleware(
    headers: HeaderMap,
    mut request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next<axum::body::Body>,
) -> Result<axum::response::Response, StatusCode> {
    // Extraire le token du header Authorization
    let auth_header = headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    if let Some(token) = auth_header {
        match auth::validate_token(token) {
            Ok(claims) => {
                // Ajouter les claims aux extensions de la requête
                request.extensions_mut().insert(claims);
                Ok(next.run(request).await)
            }
            Err(_) => Err(StatusCode::UNAUTHORIZED),
        }
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Chargement de la configuration depuis .env
    dotenvy::dotenv().ok();

    // Initialisation du logging
    tracing_subscriber::fmt()
        .with_env_filter("info,detection_system=debug,sqlx=warn")
        .init();

    // Initialisation de la base de données
    let db = database::init_database().await?;
    let state = AppState { db: db.clone() };

    // Tâche de nettoyage périodique
    let cleanup_db = db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // Chaque heure
        loop {
            interval.tick().await;
            if let Err(e) = database::cleanup_old_data(&cleanup_db, 30).await {
                tracing::error!("Erreur nettoyage: {}", e);
            }
            auth::cleanup_revoked_tokens();
        }
    });

    // Configuration CORS sécurisée
    let allowed_origins = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".to_string());
    
    let origins: Vec<HeaderValue> = allowed_origins
        .split(',')
        .filter_map(|origin| origin.trim().parse().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origins(origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
        .allow_credentials(true);

    // Configuration des routes
    let app = Router::new()
        // Routes publiques (sans authentification)
        .route("/", get(serve_index))
        .route("/api/login", post(login))
        .route("/api/health", get(health_check))
        
        // Routes protégées
        .route("/api/detection", post(add_detection))
        .route("/api/detections", get(get_detections))
        .route("/api/detections/:id", get(get_detection_by_id))
        .route("/api/stats", get(get_comprehensive_stats))
        .route("/api/export", get(export_data))
        .route("/api/logout", post(logout))
        .route("/api/change-password", post(change_password))
        .route("/api/history", get(get_detections)) // Ajout de la route history
        
        // Routes admin uniquement
        .route("/api/admin/users", post(add_user))
        .route("/api/admin/users/:username/deactivate", post(deactivate_user))
        
        // Appliquer le middleware d'auth aux routes protégées
        .layer(middleware::from_fn(auth_middleware))
        
        // Servir les fichiers statiques
        .nest_service("/static", ServeDir::new("../frontend"))
        .nest_service("/assets", ServeDir::new("../frontend/assets"))
        
        // Middlewares globaux (ordre important !)
        .layer(middleware::from_fn(security_middleware)) // Sécurité en premier
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    // Démarrage du serveur
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{}:{}", host, port);
    
    tracing::info!("🚀 Serveur démarré sur http://localhost:{}", port);
    tracing::info!("🌐 Frontend accessible sur http://localhost:{}", port);
    tracing::info!("🔐 Utilisateurs par défaut:");
    tracing::info!("   - admin / Admin123! (administrateur)");
    tracing::info!("   - viewer / Viewer123! (lecture seule)");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// === HANDLERS ===

// Page d'accueil
async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../../frontend/index.html"))
}

// Vérification de santé
async fn health_check() -> Json<ApiResponse<HashMap<String, String>>> {
    let mut info = HashMap::new();
    info.insert("status".to_string(), "healthy".to_string());
    info.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    info.insert("uptime".to_string(), format!("{:?}", std::time::SystemTime::now()));
    
    Json(ApiResponse::success(info, "Service opérationnel"))
}

// Connexion utilisateur avec protection anti-brute force
async fn login(
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(payload): Json<auth::LoginRequest>,
) -> Json<ApiResponse<auth::LoginResponse>> {
    let client_ip = addr.ip().to_string();
    
    // Vérification des tentatives de connexion
    if let Err(msg) = security::check_login_attempts(&client_ip) {
        tracing::warn!("Tentative de brute force détectée depuis {}", client_ip);
        return Json(ApiResponse::error(msg));
    }
    
    match auth::authenticate_user(&payload) {
        Ok(response) => {
            // Reset des tentatives après succès
            security::reset_login_attempts(&client_ip);
            tracing::info!("Connexion réussie: {} depuis {}", payload.username, client_ip);
            Json(ApiResponse::success(response, "Connexion réussie"))
        }
        Err(e) => {
            tracing::warn!("Échec connexion: {} depuis {} - {}", payload.username, client_ip, e);
            Json(ApiResponse::error(&format!("Erreur d'authentification: {}", e)))
        }
    }
}

// Déconnexion
async fn logout(headers: HeaderMap) -> Json<ApiResponse<()>> {
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(header_str) = auth_header.to_str() {
            if let Some(token) = header_str.strip_prefix("Bearer ") {
                let _ = auth::revoke_token(token);
            }
        }
    }
    Json(ApiResponse::success((), "Déconnexion réussie"))
}

// Changement de mot de passe
async fn change_password(
    Json(payload): Json<auth::ChangePasswordRequest>,
) -> Json<ApiResponse<()>> {
    match auth::change_password(&payload) {
        Ok(_) => Json(ApiResponse::success((), "Mot de passe modifié avec succès")),
        Err(e) => Json(ApiResponse::error(&format!("Erreur: {}", e))),
    }
}

// Ajouter une détection avec sanitisation
async fn add_detection(
    State(state): State<AppState>,
    Json(mut payload): Json<DetectionRequest>,
) -> Json<ApiResponse<database::Detection>> {
    // Validation des données
    if let Err(e) = payload.validate() {
        return Json(ApiResponse::error(&format!("Données invalides: {}", e)));
    }

    // Sanitisation des entrées
    payload.g_id = security::sanitize_input(&payload.g_id);
    payload.object_type = security::sanitize_input(&payload.object_type);
    payload.color = security::sanitize_input(&payload.color);

    match database::upsert_detection(
        &state.db,
        &payload.g_id,
        &payload.object_type,
        &payload.color,
        payload.confidence,
    )
    .await
    {
        Ok(detection) => {
            tracing::info!("Nouvelle détection: {}", payload.g_id);
            Json(ApiResponse::success(detection, "Détection enregistrée"))
        }
        Err(e) => {
            tracing::error!("Erreur insertion détection: {}", e);
            Json(ApiResponse::error(&format!("Erreur base de données: {}", e)))
        }
    }
}

// Récupérer les détections avec pagination
async fn get_detections(
    State(state): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> Json<ApiResponse<Vec<database::Detection>>> {
    match database::get_detections(
        &state.db,
        params.from_date,
        params.to_date,
        params.object_type,
        params.limit,
        params.offset,
    )
    .await
    {
        Ok(detections) => Json(ApiResponse::success(
            detections,
            "Détections récupérées avec succès",
        )),
        Err(e) => {
            tracing::error!("Erreur récupération détections: {}", e);
            Json(ApiResponse::error(&format!("Erreur: {}", e)))
        }
    }
}

// Récupérer une détection par ID
async fn get_detection_by_id(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<database::Detection>>, StatusCode> {
    let detection = sqlx::query_as!(
        database::Detection,
        r#"
        SELECT 
            id,
            g_id,
            ref_count,
            object_type,
            color,
            confidence,
            created_at as "created_at: chrono::DateTime<chrono::Local>",
            updated_at as "updated_at: chrono::DateTime<chrono::Local>"
        FROM detections 
        WHERE id = ?
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match detection {
        Some(det) => Ok(Json(ApiResponse::success(det, "Détection trouvée"))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// Statistiques complètes
async fn get_comprehensive_stats(
    State(state): State<AppState>,
) -> Json<ApiResponse<database::StatsResponse>> {
    match database::get_comprehensive_stats(&state.db).await {
        Ok(stats) => Json(ApiResponse::success(stats, "Statistiques récupérées")),
        Err(e) => {
            tracing::error!("Erreur statistiques: {}", e);
            Json(ApiResponse::error(&format!("Erreur: {}", e)))
        }
    }
}

// Export des données
async fn export_data(
    State(state): State<AppState>,
    Query(params): Query<ExportQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let format = params.format.as_deref().unwrap_or("csv");
    
    match format {
        "csv" => {
            match database::export_to_csv(&state.db, params.from_date, params.to_date).await {
                Ok(csv_content) => {
                    let headers = [
                        ("Content-Type", "text/csv; charset=utf-8"),
                        ("Content-Disposition", "attachment; filename=\"detections.csv\""),
                    ];
                    Ok((headers, csv_content))
                }
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        "json" => {
            match database::get_detections(&state.db, params.from_date, params.to_date, None, None, None).await {
                Ok(detections) => {
                    let json_content = serde_json::to_string_pretty(&detections)
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let headers = [
                        ("Content-Type", "application/json; charset=utf-8"),
                        ("Content-Disposition", "attachment; filename=\"detections.json\""),
                    ];
                    Ok((headers, json_content))
                }
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

// === ROUTES ADMIN ===

// Ajouter un utilisateur (admin uniquement)
async fn add_user(
    claims: axum::Extension<auth::Claims>,
    Json(payload): Json<HashMap<String, String>>,
) -> Json<ApiResponse<()>> {
    // Vérifier les permissions admin
    if let Err(e) = auth::check_permission(&claims, "admin") {
        return Json(ApiResponse::error(&format!("Accès refusé: {}", e)));
    }

    let username = payload.get("username").ok_or("Nom d'utilisateur manquant");
    let password = payload.get("password").ok_or("Mot de passe manquant");
    let role = payload.get("role").ok_or("Rôle manquant");

    match (username, password, role) {
        (Ok(u), Ok(p), Ok(r)) => {
            // Sanitisation des entrées
            let clean_username = security::sanitize_input(u);
            let clean_role = security::sanitize_input(r);
            
            match auth::add_user(&clean_username, p, &clean_role) {
                Ok(_) => Json(ApiResponse::success((), "Utilisateur ajouté avec succès")),
                Err(e) => Json(ApiResponse::error(&format!("Erreur: {}", e))),
            }
        }
        _ => Json(ApiResponse::error("Données manquantes")),
    }
}

// Désactiver un utilisateur (admin uniquement)
async fn deactivate_user(
    claims: axum::Extension<auth::Claims>,
    Path(username): Path<String>,
) -> Json<ApiResponse<()>> {
    // Vérifier les permissions admin
    if let Err(e) = auth::check_permission(&claims, "admin") {
        return Json(ApiResponse::error(&format!("Accès refusé: {}", e)));
    }

    let clean_username = security::sanitize_input(&username);
    
    match auth::deactivate_user(&clean_username) {
        Ok(_) => Json(ApiResponse::success((), "Utilisateur désactivé")),
        Err(e) => Json(ApiResponse::error(&format!("Erreur: {}", e))),
    }
}