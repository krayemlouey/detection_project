use axum::{
    extract::Multipart,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tower_http::cors::{CorsLayer, Any};
use tower::ServiceBuilder;
use tokio;

// Structures pour les requêtes et réponses
#[derive(Serialize, Deserialize, Debug)]
struct DetectionRequest {
    image_data: Option<String>, // base64 encoded image
    model_type: Option<String>,
    confidence: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug)]
struct DetectionResponse {
    success: bool,
    message: String,
    detections: Option<Vec<Detection>>,
    processing_time: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Detection {
    class: String,
    confidence: f32,
    bbox: BoundingBox,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BoundingBox {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

// Handler pour la route de base
async fn root() -> impl IntoResponse {
    Json(HashMap::from([
        ("status", "OK"),
        ("message", "Detection API is running"),
        ("version", "0.1.0"),
    ]))
}

// Handler pour vérifier la santé de l'API
async fn health_check() -> impl IntoResponse {
    let timestamp = chrono::Utc::now().to_rfc3339();
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": timestamp
    }))
}

// Handler principal pour la détection d'objets (avec JSON)
async fn detect_objects_json(
    Json(payload): Json<DetectionRequest>
) -> impl IntoResponse {
    println!("Received detection request: {:?}", payload);
    
    // Simulation de traitement (remplacez par votre logique réelle)
    let start_time = std::time::Instant::now();
    
    // Validation des données d'entrée
    if payload.image_data.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(DetectionResponse {
                success: false,
                message: "No image data provided".to_string(),
                detections: None,
                processing_time: None,
            })
        );
    }
    
    // Simulation de détection d'objets
    let mock_detections = vec![
        Detection {
            class: "person".to_string(),
            confidence: 0.95,
            bbox: BoundingBox {
                x: 100.0,
                y: 150.0,
                width: 200.0,
                height: 300.0,
            },
        },
        Detection {
            class: "car".to_string(),
            confidence: 0.87,
            bbox: BoundingBox {
                x: 300.0,
                y: 200.0,
                width: 250.0,
                height: 180.0,
            },
        },
    ];
    
    let processing_time = start_time.elapsed().as_secs_f32();
    
    let response = DetectionResponse {
        success: true,
        message: "Detection completed successfully".to_string(),
        detections: Some(mock_detections),
        processing_time: Some(processing_time),
    };
    
    (StatusCode::OK, Json(response))
}

// Handler pour la détection avec upload de fichier
async fn detect_objects_upload(
    mut multipart: Multipart
) -> impl IntoResponse {
    println!("Received file upload request");
    
    let start_time = std::time::Instant::now();
    let mut image_data: Option<Vec<u8>> = None;
    let mut model_type = "default".to_string();
    let mut confidence_threshold = 0.5f32;
    
    // Traitement des champs multipart
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        
        match name.as_str() {
            "image" => {
                match field.bytes().await {
                    Ok(bytes) => {
                        image_data = Some(bytes.to_vec());
                        println!("Received image data: {} bytes", bytes.len());
                    }
                    Err(e) => {
                        eprintln!("Error reading image data: {}", e);
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(DetectionResponse {
                                success: false,
                                message: format!("Error reading image: {}", e),
                                detections: None,
                                processing_time: None,
                            })
                        );
                    }
                }
            }
            "model_type" => {
                if let Ok(text) = field.text().await {
                    model_type = text;
                }
            }
            "confidence" => {
                if let Ok(text) = field.text().await {
                    if let Ok(conf) = text.parse::<f32>() {
                        confidence_threshold = conf;
                    }
                }
            }
            _ => {
                println!("Unknown field: {}", name);
            }
        }
    }
    
    // Validation
    if image_data.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(DetectionResponse {
                success: false,
                message: "No image file provided".to_string(),
                detections: None,
                processing_time: None,
            })
        );
    }
    
    // Simulation de traitement de l'image
    // Ici vous pourriez intégrer votre modèle de détection (YOLO, etc.)
    println!("Processing image with model: {}, confidence: {}", model_type, confidence_threshold);
    
    // Simulation de détections
    let mock_detections = vec![
        Detection {
            class: "person".to_string(),
            confidence: 0.92,
            bbox: BoundingBox {
                x: 120.0,
                y: 80.0,
                width: 180.0,
                height: 350.0,
            },
        },
        Detection {
            class: "bicycle".to_string(),
            confidence: 0.78,
            bbox: BoundingBox {
                x: 400.0,
                y: 250.0,
                width: 120.0,
                height: 200.0,
            },
        },
    ];
    
    let processing_time = start_time.elapsed().as_secs_f32();
    
    let response = DetectionResponse {
        success: true,
        message: format!("Image processed successfully with {} model", model_type),
        detections: Some(mock_detections),
        processing_time: Some(processing_time),
    };
    
    (StatusCode::OK, Json(response))
}

// Handler pour lister les modèles disponibles
async fn list_models() -> impl IntoResponse {
    let models = vec![
        HashMap::from([
            ("name", "yolov8n"),
            ("description", "YOLOv8 Nano - Fast and lightweight"),
            ("size", "6MB"),
        ]),
        HashMap::from([
            ("name", "yolov8s"),
            ("description", "YOLOv8 Small - Good balance of speed and accuracy"),
            ("size", "22MB"),
        ]),
        HashMap::from([
            ("name", "yolov8m"),
            ("description", "YOLOv8 Medium - Higher accuracy"),
            ("size", "52MB"),
        ]),
    ];
    
    Json(HashMap::from([
        ("success", serde_json::Value::Bool(true)),
        ("models", serde_json::to_value(models).unwrap()),
    ]))
}

// Configuration CORS
fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}

// Fonction principale
#[tokio::main]
async fn main() {
    // Initialisation des logs
    env_logger::init();
    
    println!("🚀 Starting Detection API Server...");
    
    // Configuration des routes
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/detect", post(detect_objects_json))
        .route("/detect/upload", post(detect_objects_upload))
        .route("/models", get(list_models))
        .layer(
            ServiceBuilder::new()
                .layer(cors_layer())
        );
    
    // Configuration du serveur
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind to address");
        
    println!("✅ Server running on http://127.0.0.1:3000");
    println!("📖 Available endpoints:");
    println!("  GET  /           - API status");
    println!("  GET  /health     - Health check");
    println!("  POST /detect     - Object detection (JSON)");
    println!("  POST /detect/upload - Object detection (File upload)");
    println!("  GET  /models     - List available models");
    
    // Démarrage du serveur
    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}