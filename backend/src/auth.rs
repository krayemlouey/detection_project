///auth.rs
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Local};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use anyhow::{Result, anyhow};
use validator::Validate;

// Configuration JWT
const JWT_SECRET: &[u8] = b"your-super-secret-jwt-key-change-in-production";
const TOKEN_EXPIRY_HOURS: i64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // Subject (username)
    pub exp: usize,       // Expiration
    pub iat: usize,       // Issued at
    pub role: String,     // User role
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 3, max = 50, message = "Le nom d'utilisateur doit contenir entre 3 et 50 caractères"))]
    pub username: String,
    
    #[validate(length(min = 6, message = "Le mot de passe doit contenir au moins 6 caractères"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 3, max = 50))]
    pub username: String,
    
    pub current_password: String,
    
    #[validate(length(min = 8, message = "Le nouveau mot de passe doit contenir au moins 8 caractères"))]
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub username: String,
    pub role: String,
}

// Base de données d'utilisateurs (en production, utilisez une vraie DB)
lazy_static::lazy_static! {
    static ref USERS: std::sync::RwLock<std::collections::HashMap<String, User>> = {
        let mut users = std::collections::HashMap::new();
        
        // Utilisateur admin par défaut
        users.insert(
            "admin".to_string(),
            User {
                username: "admin".to_string(),
                password_hash: hash("Admin123!", DEFAULT_COST).unwrap(),
                role: "admin".to_string(),
                is_active: true,
            }
        );
        
        // Utilisateur viewer par défaut
        users.insert(
            "viewer".to_string(),
            User {
                username: "viewer".to_string(),
                password_hash: hash("Viewer123!", DEFAULT_COST).unwrap(),
                role: "viewer".to_string(),
                is_active: true,
            }
        );
        
        std::sync::RwLock::new(users)
    };
    
    // Liste des tokens révoqués (blacklist)
    static ref REVOKED_TOKENS: std::sync::RwLock<HashSet<String>> = {
        std::sync::RwLock::new(HashSet::new())
    };
}

/// Authentifie un utilisateur et génère un JWT
pub fn authenticate_user(credentials: &LoginRequest) -> Result<LoginResponse> {
    // Validation des données d'entrée
    credentials.validate()
        .map_err(|e| anyhow!("Données invalides: {}", e))?;

    let users = USERS.read().unwrap();
    
    let user = users.get(&credentials.username)
        .ok_or_else(|| anyhow!("Utilisateur non trouvé"))?;

    if !user.is_active {
        return Err(anyhow!("Compte désactivé"));
    }

    // Vérification du mot de passe
    if !verify(&credentials.password, &user.password_hash)? {
        return Err(anyhow!("Mot de passe incorrect"));
    }

    // Génération du JWT
    let now = Local::now();
    let expiry = now + Duration::hours(TOKEN_EXPIRY_HOURS);
    
    let claims = Claims {
        sub: user.username.clone(),
        exp: expiry.timestamp() as usize,
        iat: now.timestamp() as usize,
        role: user.role.clone(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )?;

    tracing::info!("✅ Utilisateur connecté: {}", user.username);

    Ok(LoginResponse {
        token,
        expires_at: expiry.format("%Y-%m-%d %H:%M:%S").to_string(),
        user: UserInfo {
            username: user.username.clone(),
            role: user.role.clone(),
        },
    })
}

/// Valide un token JWT
pub fn validate_token(token: &str) -> Result<Claims> {
    // Vérifier si le token est dans la blacklist
    let revoked_tokens = REVOKED_TOKENS.read().unwrap();
    if revoked_tokens.contains(token) {
        return Err(anyhow!("Token révoqué"));
    }

    // Décoder et valider le token
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET),
        &Validation::default(),
    )?;

    // Vérifier que l'utilisateur existe toujours
    let users = USERS.read().unwrap();
    let user = users.get(&token_data.claims.sub)
        .ok_or_else(|| anyhow!("Utilisateur non trouvé"))?;

    if !user.is_active {
        return Err(anyhow!("Compte désactivé"));
    }

    Ok(token_data.claims)
}

/// Révoque un token (logout)
pub fn revoke_token(token: &str) -> Result<()> {
    let mut revoked_tokens = REVOKED_TOKENS.write().unwrap();
    revoked_tokens.insert(token.to_string());
    
    tracing::info!("🔐 Token révoqué");
    Ok(())
}

/// Change le mot de passe d'un utilisateur
pub fn change_password(request: &ChangePasswordRequest) -> Result<()> {
    // Validation
    request.validate()
        .map_err(|e| anyhow!("Données invalides: {}", e))?;

    let mut users = USERS.write().unwrap();
    
    let user = users.get_mut(&request.username)
        .ok_or_else(|| anyhow!("Utilisateur non trouvé"))?;

    // Vérifier le mot de passe actuel
    if !verify(&request.current_password, &user.password_hash)? {
        return Err(anyhow!("Mot de passe actuel incorrect"));
    }

    // Hash du nouveau mot de passe
    user.password_hash = hash(&request.new_password, DEFAULT_COST)?;
    
    tracing::info!("🔒 Mot de passe changé pour: {}", request.username);
    Ok(())
}

/// Vérifie les permissions d'un utilisateur
pub fn check_permission(claims: &Claims, required_role: &str) -> Result<()> {
    match (claims.role.as_str(), required_role) {
        ("admin", _) => Ok(()), // Admin a tous les droits
        ("viewer", "viewer") => Ok(()),
        ("viewer", "read") => Ok(()),
        _ => Err(anyhow!("Permissions insuffisantes")),
    }
}

/// Ajoute un nouvel utilisateur (admin uniquement)
pub fn add_user(username: &str, password: &str, role: &str) -> Result<()> {
    if username.len() < 3 || username.len() > 50 {
        return Err(anyhow!("Le nom d'utilisateur doit contenir entre 3 et 50 caractères"));
    }
    
    if password.len() < 8 {
        return Err(anyhow!("Le mot de passe doit contenir au moins 8 caractères"));
    }
    
    if !["admin", "viewer"].contains(&role) {
        return Err(anyhow!("Rôle invalide"));
    }

    let mut users = USERS.write().unwrap();
    
    if users.contains_key(username) {
        return Err(anyhow!("L'utilisateur existe déjà"));
    }

    let user = User {
        username: username.to_string(),
        password_hash: hash(password, DEFAULT_COST)?,
        role: role.to_string(),
        is_active: true,
    };

    users.insert(username.to_string(), user);
    
    tracing::info!("👤 Nouvel utilisateur ajouté: {} ({})", username, role);
    Ok(())
}

/// Désactive un utilisateur
pub fn deactivate_user(username: &str) -> Result<()> {
    let mut users = USERS.write().unwrap();
    
    let user = users.get_mut(username)
        .ok_or_else(|| anyhow!("Utilisateur non trouvé"))?;

    user.is_active = false;
    
    tracing::warn!("🚫 Utilisateur désactivé: {}", username);
    Ok(())
}

/// Nettoie les tokens expirés de la blacklist
pub fn cleanup_revoked_tokens() {
    let mut revoked_tokens = REVOKED_TOKENS.write().unwrap();
    
    // En production, vous devriez stocker les timestamps des tokens
    // et nettoyer seulement ceux qui sont expirés
    let initial_count = revoked_tokens.len();
    
    // Pour cet exemple, on garde seulement les 1000 derniers tokens
    if revoked_tokens.len() > 1000 {
        let tokens_to_keep: HashSet<String> = revoked_tokens
            .iter()
            .take(1000)
            .cloned()
            .collect();
        *revoked_tokens = tokens_to_keep;
    }
    
    let final_count = revoked_tokens.len();
    if initial_count != final_count {
        tracing::info!("🧹 Nettoyage tokens: {} -> {}", initial_count, final_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_authentication() {
        let credentials = LoginRequest {
            username: "admin".to_string(),
            password: "Admin123!".to_string(),
        };
        
        let result = authenticate_user(&credentials);
        assert!(result.is_ok());
        
        let login_response = result.unwrap();
        assert_eq!(login_response.user.username, "admin");
        assert_eq!(login_response.user.role, "admin");
    }

    #[test]
    fn test_invalid_credentials() {
        let credentials = LoginRequest {
            username: "admin".to_string(),
            password: "wrongpassword".to_string(),
        };
        
        let result = authenticate_user(&credentials);
        assert!(result.is_err());
    }

    #[test]
    fn test_token_validation() {
        let credentials = LoginRequest {
            username: "admin".to_string(),
            password: "Admin123!".to_string(),
        };
        
        let login_response = authenticate_user(&credentials).unwrap();
        let validation_result = validate_token(&login_response.token);
        
        assert!(validation_result.is_ok());
        let claims = validation_result.unwrap();
        assert_eq!(claims.sub, "admin");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_permission_check() {
        let admin_claims = Claims {
            sub: "admin".to_string(),
            exp: 9999999999,
            iat: 1234567890,
            role: "admin".to_string(),
        };
        
        let viewer_claims = Claims {
            sub: "viewer".to_string(),
            exp: 9999999999,
            iat: 1234567890,
            role: "viewer".to_string(),
        };
        
        // Admin peut tout faire
        assert!(check_permission(&admin_claims, "admin").is_ok());
        assert!(check_permission(&admin_claims, "viewer").is_ok());
        
        // Viewer ne peut que lire
        assert!(check_permission(&viewer_claims, "viewer").is_ok());
        assert!(check_permission(&viewer_claims, "admin").is_err());
    }
}