use anyhow::Result;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use shared::{Claims, PaymentError};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct AuthService {
    secret_key: String,
}

impl AuthService {
    pub fn new() -> Self {
        Self {
            secret_key: std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret_key".to_string()),
        }
    }

    pub fn generate_token(&self, subject: String) -> Result<String, PaymentError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let claims = Claims {
            sub: subject,
            exp: now + 3600, // 1 hour
            iat: now,
            iss: "payment-processor".to_string(),
            aud: "dfsp-lite".to_string(),
        };

        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(self.secret_key.as_ref()),
        )
        .map_err(|e| PaymentError::AuthError(e.to_string()))
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, PaymentError> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret_key.as_ref()),
            &validation,
        )
        .map_err(|e| PaymentError::AuthError(e.to_string()))?;

        Ok(token_data.claims)
    }
}



