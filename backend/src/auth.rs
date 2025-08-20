use axum::{
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use chrono::{Utc, Duration};
use sha2::{Sha256, Digest};

// Configuration par défaut (à changer en production)
const DEFAULT_USERNAME: &str = "admin";
const DEFAULT_PASSWORD: &str = "password123";
const JWT_SECRET: &str = "your-super-secret-jwt-key-change-in-production-123456789";

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: i64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenRequest {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,  // Subject (user identifier)
    pub exp: i64,     // Expiration time
    pub iat: i64,     // Issued at
    pub role: String, // User role
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "Success".to_string(),
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            message: message.to_string(),
        }
    }
}

// Fonction de hashage des mots de passe
fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

// Vérifier les identifiants utilisateur
fn verify_user_credentials(username: &str, password: &str) -> bool {
    // En production, ceci devrait vérifier contre une base de données
    if username == DEFAULT_USERNAME {
        let hashed_input = hash_password(password);
        let hashed_default = hash_password(DEFAULT_PASSWORD);
        return hashed_input == hashed_default;
    }
    
    // Ajouter d'autres utilisateurs ici si nécessaire
    false
}

// Générer un JWT token
fn generate_jwt_token(username: &str, role: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let expires_at = now + Duration::hours(24); // Token valide 24h

    let claims = Claims {
        sub: username.to_string(),
        exp: expires_at.timestamp(),
        iat: now.timestamp(),
        role: role.to_string(),
    };

    let header = Header::new(Algorithm::HS256);
    let encoding_key = EncodingKey::from_secret(JWT_SECRET.as_bytes());

    encode(&header, &claims, &encoding_key)
}

// Vérifier un JWT token
pub fn verify_jwt_token(token: &str) -> bool {
    let decoding_key = DecodingKey::from_secret(JWT_SECRET.as_bytes());
    let validation = Validation::new(Algorithm::HS256);

    match decode::<Claims>(token, &decoding_key, &validation) {
        Ok(token_data) => {
            // Vérifier si le token n'est pas expiré
            let now = Utc::now().timestamp();
            if token_data.claims.exp < now {
                println!("🔒 Token expiré pour l'utilisateur: {}", token_data.claims.sub);
                return false;
            }
            
            println!("✅ Token valide pour l'utilisateur: {}", token_data.claims.sub);
            true
        }
        Err(e) => {
            println!("❌ Token invalide: {}", e);
            false
        }
    }
}

// Extraire les informations du token
pub fn extract_user_from_token(token: &str) -> Option<UserInfo> {
    let decoding_key = DecodingKey::from_secret(JWT_SECRET.as_bytes());
    let validation = Validation::new(Algorithm::HS256);

    match decode::<Claims>(token, &decoding_key, &validation) {
        Ok(token_data) => {
            let now = Utc::now().timestamp();
            if token_data.claims.exp >= now {
                Some(UserInfo {
                    username: token_data.claims.sub,
                    role: token_data.claims.role,
                })
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

// Route de connexion
pub async fn login(
    Json(login_request): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, (StatusCode, Json<ApiResponse<LoginResponse>>)> {
    println!("🔐 Tentative de connexion pour: {}", login_request.username);

    // Vérifier les identifiants
    if !verify_user_credentials(&login_request.username, &login_request.password) {
        println!("❌ Identifiants invalides pour: {}", login_request.username);
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Invalid username or password")),
        ));
    }

    // Déterminer le rôle (en production, ceci viendrait de la DB)
    let role = if login_request.username == DEFAULT_USERNAME {
        "admin"
    } else {
        "user"
    };

    // Générer le token JWT
    match generate_jwt_token(&login_request.username, role) {
        Ok(token) => {
            let expires_at = (Utc::now() + Duration::hours(24)).timestamp();
            
            let response = LoginResponse {
                token: token.clone(),
                expires_at,
                user: UserInfo {
                    username: login_request.username.clone(),
                    role: role.to_string(),
                },
            };

            println!("✅ Connexion réussie pour: {} (rôle: {})", login_request.username, role);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            eprintln!("❌ Erreur lors de la génération du token: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to generate authentication token")),
            ))
        }
    }
}

// Route de vérification de token
pub async fn verify_token(
    Json(token_request): Json<TokenRequest>,
) -> Result<Json<ApiResponse<UserInfo>>, (StatusCode, Json<ApiResponse<UserInfo>>)> {
    println!("🔍 Vérification du token...");

    if verify_jwt_token(&token_request.token) {
        if let Some(user_info) = extract_user_from_token(&token_request.token) {
            println!("✅ Token valide pour: {}", user_info.username);
            Ok(Json(ApiResponse::success(user_info)))
        } else {
            println!("❌ Impossible d'extraire les infos utilisateur du token");
            Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::error("Invalid token format")),
            ))
        }
    } else {
        println!("❌ Token invalide ou expiré");
        Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Invalid or expired token")),
        ))
    }
}

// Middleware pour vérifier l'authentification (à utiliser dans les routes protégées)
pub fn require_auth(token: &str) -> Result<UserInfo, &'static str> {
    if verify_jwt_token(token) {
        if let Some(user_info) = extract_user_from_token(token) {
            Ok(user_info)
        } else {
            Err("Invalid token format")
        }
    } else {
        Err("Invalid or expired token")
    }
}

// Fonction pour créer un nouvel utilisateur (pour usage futur)
pub fn create_user(username: &str, password: &str, role: &str) -> Result<UserInfo, &'static str> {
    // En production, ceci sauvegarderait dans la base de données
    if username.len() < 3 {
        return Err("Username must be at least 3 characters long");
    }
    
    if password.len() < 6 {
        return Err("Password must be at least 6 characters long");
    }
    
    let valid_roles = ["admin", "user", "viewer"];
    if !valid_roles.contains(&role) {
        return Err("Invalid role specified");
    }
    
    // Ici on sauvegarderait l'utilisateur avec le mot de passe hashé
    let _hashed_password = hash_password(password);
    
    println!("👤 Nouvel utilisateur créé: {} (rôle: {})", username, role);
    
    Ok(UserInfo {
        username: username.to_string(),
        role: role.to_string(),
    })
}

// Fonction pour changer le mot de passe (pour usage futur)
pub fn change_password(username: &str, old_password: &str, new_password: &str) -> Result<bool, &'static str> {
    // Vérifier l'ancien mot de passe
    if !verify_user_credentials(username, old_password) {
        return Err("Invalid current password");
    }
    
    if new_password.len() < 6 {
        return Err("New password must be at least 6 characters long");
    }
    
    // En production, ceci mettrait à jour la base de données
    let _new_hashed_password = hash_password(new_password);
    
    println!("🔑 Mot de passe changé pour l'utilisateur: {}", username);
    Ok(true)
}

// Configuration des en-têtes de sécurité
pub fn security_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("X-Frame-Options", "DENY"),
        ("X-Content-Type-Options", "nosniff"),
        ("X-XSS-Protection", "1; mode=block"),
        ("Referrer-Policy", "strict-origin-when-cross-origin"),
        ("Content-Security-Policy", "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "test123";
        let hash1 = hash_password(password);
        let hash2 = hash_password(password);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_user_credentials() {
        assert!(verify_user_credentials(DEFAULT_USERNAME, DEFAULT_PASSWORD));
        assert!(!verify_user_credentials("wrong_user", "wrong_pass"));
        assert!(!verify_user_credentials(DEFAULT_USERNAME, "wrong_pass"));
    }

    #[test]
    fn test_jwt_token_generation() {
        let token = generate_jwt_token("test_user", "admin").unwrap();
        assert!(!token.is_empty());
        assert!(verify_jwt_token(&token));
    }

    #[test]
    fn test_user_extraction_from_token() {
        let token = generate_jwt_token("test_user", "admin").unwrap();
        let user_info = extract_user_from_token(&token).unwrap();
        assert_eq!(user_info.username, "test_user");
        assert_eq!(user_info.role, "admin");
    }
}