// src/security.rs - Module de sécurité (version minimale)
use axum::{
    extract::Request,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Cache simple pour les tentatives de connexion (remplace DashMap pour éviter les dépendances)
lazy_static::lazy_static! {
    static ref LOGIN_ATTEMPTS: Arc<Mutex<HashMap<String, (u32, Instant)>>> = 
        Arc::new(Mutex::new(HashMap::new()));
}

/// Middleware de sécurité général
pub async fn security_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Traitement de la requête
    let mut response = next.run(request).await;
    
    // Ajouter les headers de sécurité
    add_security_headers(response.headers_mut());
    
    Ok(response)
}

/// Ajoute les headers de sécurité essentiels
pub fn add_security_headers(headers: &mut HeaderMap) {
    // Content Security Policy strict
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'unsafe-inline' https://cdnjs.cloudflare.com; \
             style-src 'self' 'unsafe-inline' https://cdnjs.cloudflare.com; \
             img-src 'self' data: https:; \
             connect-src 'self'; \
             font-src 'self' https://cdnjs.cloudflare.com; \
             object-src 'none'; \
             media-src 'none'; \
             frame-src 'none';"
        ),
    );

    // Headers de sécurité essentiels
    let security_headers = [
        ("x-frame-options", "DENY"),
        ("x-content-type-options", "nosniff"),
        ("x-xss-protection", "1; mode=block"),
        ("strict-transport-security", "max-age=31536000; includeSubDomains"),
        ("referrer-policy", "strict-origin-when-cross-origin"),
        ("permissions-policy", "camera=(), microphone=(), geolocation=()"),
    ];

    for (name, value) in security_headers.iter() {
        if let (Ok(header_name), Ok(header_value)) = (
            HeaderName::from_static(name),
            HeaderValue::from_static(value)
        ) {
            headers.insert(header_name, header_value);
        }
    }
}

/// Validation des tentatives de login (protection anti-brute force)
pub fn check_login_attempts(ip: &str) -> Result<(), &'static str> {
    const MAX_ATTEMPTS: u32 = 5;
    const LOCKOUT_DURATION: Duration = Duration::from_secs(300); // 5 minutes

    let now = Instant::now();
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap();
    
    if let Some((count, last_attempt)) = attempts.get(ip) {
        if now.duration_since(*last_attempt) > LOCKOUT_DURATION {
            // Reset après expiration
            attempts.insert(ip.to_string(), (1, now));
            Ok(())
        } else if *count >= MAX_ATTEMPTS {
            Err("Trop de tentatives de connexion. Réessayez dans 5 minutes.")
        } else {
            // Incrémenter les tentatives
            attempts.insert(ip.to_string(), (*count + 1, now));
            Ok(())
        }
    } else {
        // Première tentative pour cette IP
        attempts.insert(ip.to_string(), (1, now));
        Ok(())
    }
}

/// Reset des tentatives après connexion réussie
pub fn reset_login_attempts(ip: &str) {
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap();
    attempts.remove(ip);
}

/// Sanitisation basique des entrées utilisateur
pub fn sanitize_input(input: &str) -> String {
    // Sanitisation basique pour éviter les injections
    input
        .chars()
        .filter(|c| c.is_alphanumeric() || "_-./: ".contains(*c))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Génération de tokens sécurisés simples
pub fn generate_secure_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    
    format!("token_{}", timestamp)
}

/// Comparaison sécurisée basique
pub fn secure_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut result = 0u8;
    for (byte_a, byte_b) in a.bytes().zip(b.bytes()) {
        result |= byte_a ^ byte_b;
    }
    
    result == 0
}

/// Nettoyage périodique des tentatives de connexion expirées
pub fn cleanup_login_attempts() {
    const CLEANUP_DURATION: Duration = Duration::from_secs(600); // 10 minutes
    
    let now = Instant::now();
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap();
    
    attempts.retain(|_, (_, last_attempt)| {
        now.duration_since(*last_attempt) <= CLEANUP_DURATION
    });
    
    tracing::debug!("Nettoyage des tentatives de connexion expirées");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_input() {
        assert_eq!(sanitize_input("test<script>"), "testscript");
        assert_eq!(sanitize_input("user@domain.com"), "user@domain.com");
        assert_eq!(sanitize_input("  spaced  "), "spaced");
    }

    #[test]
    fn test_secure_compare() {
        assert!(secure_compare("test", "test"));
        assert!(!secure_compare("test", "wrong"));
        assert!(!secure_compare("short", "longer"));
    }

    #[test]
    fn test_login_attempts() {
        let ip = "127.0.0.1";
        
        // Première tentative
        assert!(check_login_attempts(ip).is_ok());
        
        // Reset
        reset_login_attempts(ip);
        assert!(check_login_attempts(ip).is_ok());
    }
}