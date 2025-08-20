use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use warp::Filter;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

mod database;
mod auth;

use database::Database;
use auth::{verify_admin, LoginRequest, LoginResponse};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Detection {
    id: Option<i32>,
    g_id: String,
    object_type: String,
    color: String,
    datetime: String,
    ref_count: Option<i32>,
    area: Option<i32>,
    center_x: Option<i32>,
    center_y: Option<i32>,
    image_filename: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse {
    success: bool,
    message: String,
    data: Option<serde_json::Value>,
}

type SharedDb = Arc<RwLock<Database>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialiser la base de données
    let db = Arc::new(RwLock::new(Database::new("detection.db").await?));
    
    // Routes CORS
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type", "authorization"])
        .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS"]);

    // Route de login (POST /api/login)
    let login_route = warp::path("api")
        .and(warp::path("login"))
        .and(warp::post())
        .and(warp::body::json())
        .map(|login_req: LoginRequest| {
            match auth::authenticate(login_req) {
                Ok(response) => warp::reply::with_status(
                    warp::reply::json(&response), 
                    warp::http::StatusCode::OK
                ),
                Err(_) => warp::reply::with_status(
                    warp::reply::json(&LoginResponse {
                        success: false,
                        token: None,
                        message: Some("Identifiants incorrects".to_string()),
                    }),
                    warp::http::StatusCode::UNAUTHORIZED
                )
            }
        });

    // Route pour ajouter une détection (POST /api/detections)
    let db_add = db.clone();
    let add_detection_route = warp::path("api")
        .and(warp::path("detections"))
        .and(warp::post())
        .and(warp::body::json())
        .and_then(move |detection: Detection| {
            let db = db_add.clone();
            async move {
                let mut db_lock = db.write().await;
                match db_lock.insert_detection(detection).await {
                    Ok(id) => Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: true,
                            message: "Détection ajoutée avec succès".to_string(),
                            data: Some(serde_json::json!({ "id": id })),
                        }),
                        warp::http::StatusCode::CREATED,
                    )),
                    Err(e) => Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: false,
                            message: format!("Erreur: {}", e),
                            data: None,
                        }),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )),
                }
            }
        });

    // Route pour obtenir toutes les détections (GET /api/detections)
    let db_get = db.clone();
    let get_detections_route = warp::path("api")
        .and(warp::path("detections"))
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth_header: Option<String>| {
            let db = db_get.clone();
            async move {
                // Vérifier l'authentification pour l'historique
                if let Some(header) = auth_header {
                    if let Some(token) = header.strip_prefix("Bearer ") {
                        if !verify_admin(token) {
                            return Ok(warp::reply::with_status(
                                warp::reply::json(&ApiResponse {
                                    success: false,
                                    message: "Accès non autorisé".to_string(),
                                    data: None,
                                }),
                                warp::http::StatusCode::UNAUTHORIZED,
                            ));
                        }
                    } else {
                        return Ok(warp::reply::with_status(
                            warp::reply::json(&ApiResponse {
                                success: false,
                                message: "Token manquant".to_string(),
                                data: None,
                            }),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ));
                    }
                } else {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: false,
                            message: "Authentification requise".to_string(),
                            data: None,
                        }),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ));
                }

                let db_lock = db.read().await;
                match db_lock.get_all_detections().await {
                    Ok(detections) => Ok(warp::reply::with_status(
                        warp::reply::json(&detections),
                        warp::http::StatusCode::OK,
                    )),
                    Err(e) => Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: false,
                            message: format!("Erreur: {}", e),
                            data: None,
                        }),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )),
                }
            }
        });

    // Route pour supprimer une détection (DELETE /api/detections/:id)
    let db_delete = db.clone();
    let delete_detection_route = warp::path("api")
        .and(warp::path("detections"))
        .and(warp::path::param::<i32>())
        .and(warp::delete())
        .and(warp::header::<String>("authorization"))
        .and_then(move |id: i32, auth_header: String| {
            let db = db_delete.clone();
            async move {
                // Vérifier l'authentification admin
                if let Some(token) = auth_header.strip_prefix("Bearer ") {
                    if !verify_admin(token) {
                        return Ok(warp::reply::with_status(
                            warp::reply::json(&ApiResponse {
                                success: false,
                                message: "Accès non autorisé".to_string(),
                                data: None,
                            }),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ));
                    }
                } else {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: false,
                            message: "Token invalide".to_string(),
                            data: None,
                        }),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ));
                }

                let mut db_lock = db.write().await;
                match db_lock.delete_detection(id).await {
                    Ok(_) => Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: true,
                            message: "Détection supprimée avec succès".to_string(),
                            data: None,
                        }),
                        warp::http::StatusCode::OK,
                    )),
                    Err(e) => Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: false,
                            message: format!("Erreur: {}", e),
                            data: None,
                        }),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )),
                }
            }
        });

    // Route pour reset database (POST /api/reset) - ADMIN SEULEMENT
    let db_reset = db.clone();
    let reset_database_route = warp::path("api")
        .and(warp::path("reset"))
        .and(warp::post())
        .and(warp::header::<String>("authorization"))
        .and_then(move |auth_header: String| {
            let db = db_reset.clone();
            async move {
                // Vérifier l'authentification admin
                if let Some(token) = auth_header.strip_prefix("Bearer ") {
                    if !verify_admin(token) {
                        return Ok(warp::reply::with_status(
                            warp::reply::json(&ApiResponse {
                                success: false,
                                message: "Accès non autorisé - Admin requis".to_string(),
                                data: None,
                            }),
                            warp::http::StatusCode::FORBIDDEN,
                        ));
                    }
                } else {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: false,
                            message: "Token manquant".to_string(),
                            data: None,
                        }),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ));
                }

                let mut db_lock = db.write().await;
                match db_lock.reset_database().await {
                    Ok(_) => Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: true,
                            message: "Base de données réinitialisée avec succès".to_string(),
                            data: None,
                        }),
                        warp::http::StatusCode::OK,
                    )),
                    Err(e) => Ok(warp::reply::with_status(
                        warp::reply::json(&ApiResponse {
                            success: false,
                            message: format!("Erreur lors de la réinitialisation: {}", e),
                            data: None,
                        }),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )),
                }
            }
        });

    // Route pour servir les fichiers statiques
    let static_files = warp::fs::dir("../frontend");

    // Combiner toutes les routes
    let api_routes = login_route
        .or(add_detection_route)
        .or(get_detections_route)
        .or(delete_detection_route)
        .or(reset_database_route);

    let routes = api_routes
        .or(static_files)
        .with(cors)
        .recover(handle_rejection);

    println!("🚀 Serveur démarré sur http://localhost:3000");
    println!("📋 Dashboard: http://localhost:3000");
    println!("🔐 Login: http://localhost:3000/login.html");
    println!("📊 Historique: http://localhost:3000/history.html");
    
    warp::serve(routes)
        .run(([0, 0, 0, 0], 3000))
        .await;

    Ok(())
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = warp::http::StatusCode::NOT_FOUND;
        message = "Route non trouvée";
    } else if let Some(_) = err.find::<warp::filters::body::BodyDeserializeError>() {
        code = warp::http::StatusCode::BAD_REQUEST;
        message = "Corps de requête invalide";
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        code = warp::http::StatusCode::METHOD_NOT_ALLOWED;
        message = "Méthode non autorisée";
    } else {
        code = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
        message = "Erreur interne du serveur";
    }

    let json = warp::reply::json(&ApiResponse {
        success: false,
        message: message.to_string(),
        data: None,
    });

    Ok(warp::reply::with_status(json, code))
}