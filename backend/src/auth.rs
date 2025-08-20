use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// Identifiants par défaut - À MODIFIER pour la production
const DEFAULT_USERNAME: &str = "admin";
const DEFAULT_PASSWORD: &str = "password123";

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<String>,
    pub message: Option<String>,
}

// Simple token storage - En production, utiliser Redis ou une DB
static mut VALID_TOKENS: Option<HashMap<String, u64>> = None;

fn init_tokens() {
    unsafe {
        if VALID_TOKENS.is_none() {
            VALID_TOKENS = Some(HashMap::new());
        }
    }
}

pub fn authenticate(login_req: LoginRequest) -> Result<LoginResponse, &'static str> {
    init_tokens();
    
    // Validation basique
    if login_req.username.trim().is_empty() || login_req.password.is_empty() {
        return Err("Champs manquants");
    }

    // Vérification des identifiants (simple mais sécurisé pour ce cas)
    if login_req.username == DEFAULT_USERNAME && login_req.password == DEFAULT_PASSWORD {
        // Générer un token simple mais unique
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let token = format!("admin_token_{}_{}", timestamp, generate_random_suffix());
        
        // Stocker le token avec timestamp d'expiration (24h)
        unsafe {
            if let Some(ref mut tokens) = VALID_TOKENS {
                tokens.insert(token.clone(), timestamp + 86400); // 24h d'expiration
            }
        }
        
        Ok(LoginResponse {
            success: true,
            token: Some(token),
            message: Some("Connexion réussie".to_string()),
        })
    } else {
        Err("Identifiants incorrects")
    }
}

pub fn verify_admin(token: &str) -> bool {
    init_tokens();
    
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    unsafe {
        if let Some(ref mut tokens) = VALID_TOKENS {
            if let Some(&expiry) = tokens.get(token) {
                if current_time < expiry {
                    return true;
                } else {
                    // Token expiré, le supprimer
                    tokens.remove(token);
                }
            }
        }
    }
    
    false
}

fn generate_random_suffix() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    format!("{:x}", hasher.finish() % 1000000)
}

// Fonction pour nettoyer les tokens expirés (appelée périodiquement)
pub fn cleanup_expired_tokens() {
    init_tokens();
    
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    unsafe {
        if let Some(ref mut tokens) = VALID_TOKENS {
            tokens.retain(|_, &mut expiry| current_time < expiry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_login() {
        let login_req = LoginRequest {
            username: DEFAULT_USERNAME.to_string(),
            password: DEFAULT_PASSWORD.to_string(),
        };
        
        let result = authenticate(login_req);
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.success);
        assert!(response.token.is_some());
    }

    #[test]
    fn test_invalid_login() {
        let login_req = LoginRequest {
            username: "wrong".to_string(),
            password: "wrong".to_string(),
        };
        
        let result = authenticate(login_req);
        assert!(result.is_err());
    }

    #[test]
    fn test_token_verification() {
        let login_req = LoginRequest {
            username: DEFAULT_USERNAME.to_string(),
            password: DEFAULT_PASSWORD.to_string(),
        };
        
        let response = authenticate(login_req).unwrap();
        let token = response.token.unwrap();
        
        assert!(verify_admin(&token));
        assert!(!verify_admin("invalid_token"));
    }
}