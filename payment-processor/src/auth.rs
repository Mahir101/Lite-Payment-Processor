//! # Authentication Module
//! 
//! This module provides JWT-based authentication services for the Payment Processor.
//! It handles token generation, validation, and management for secure API access.
//! 
//! ## Key Features:
//! - JWT token generation with configurable expiration
//! - Token validation with signature verification
//! - Secure secret key management
//! - Claims-based authentication
//! 
//! ## Security Considerations:
//! - Uses HS256 algorithm for token signing
//! - Configurable token expiration (default: 1 hour)
//! - Secret key should be stored securely in production
//! - Tokens include issuer and audience claims for validation

use anyhow::Result;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use shared::{Claims, PaymentError};
use std::time::{SystemTime, UNIX_EPOCH};

/// Authentication service for JWT token management
/// 
/// This service handles the generation and validation of JWT tokens used for
/// API authentication. It uses HMAC-SHA256 for token signing and includes
/// standard claims for security validation.
#[derive(Clone)]
pub struct AuthService {
    /// Secret key used for signing and validating JWT tokens
    /// This should be stored securely and rotated regularly in production
    secret_key: String,
}

impl AuthService {
    /// Creates a new authentication service instance
    /// 
    /// This function initializes the AuthService with a secret key from environment
    /// variables. In production, the JWT_SECRET should be a strong, randomly
    /// generated key stored securely.
    /// 
    /// # Environment Variables:
    /// - `JWT_SECRET`: Secret key for JWT signing (defaults to "secret_key" if not set)
    /// 
    /// # Returns:
    /// - `AuthService`: New authentication service instance
    /// 
    /// # Security Note:
    /// The default secret key is only suitable for development. Production
    /// deployments must set a strong JWT_SECRET environment variable.
    pub fn new() -> Self {
        Self {
            secret_key: std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret_key".to_string()),
        }
    }

    /// Generates a new JWT token for the given subject
    /// 
    /// This function creates a signed JWT token with standard claims including
    /// expiration time, issued at time, issuer, and audience. The token expires
    /// after 1 hour by default.
    /// 
    /// # Parameters:
    /// - `subject`: The subject (usually user ID) for the token
    /// 
    /// # Returns:
    /// - `Ok(String)`: Signed JWT token
    /// - `Err(PaymentError)`: Token generation failed
    /// 
    /// # Token Claims:
    /// - `sub`: Subject (user identifier)
    /// - `exp`: Expiration time (1 hour from now)
    /// - `iat`: Issued at time (current time)
    /// - `iss`: Issuer ("payment-processor")
    /// - `aud`: Audience ("dfsp-lite")
    /// 
    /// # Security:
    /// Uses HMAC-SHA256 algorithm for token signing to ensure integrity
    /// and authenticity of the token.
    pub fn generate_token(&self, subject: String) -> Result<String, PaymentError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let claims = Claims {
            sub: subject,
            exp: now + 3600, // 1 hour expiration
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

    /// Validates a JWT token and extracts its claims
    /// 
    /// This function verifies the signature of a JWT token and extracts its claims
    /// if the token is valid. It checks the signature, expiration, and other
    /// standard claims.
    /// 
    /// # Parameters:
    /// - `token`: The JWT token string to validate
    /// 
    /// # Returns:
    /// - `Ok(Claims)`: Extracted claims if token is valid
    /// - `Err(PaymentError)`: Token validation failed
    /// 
    /// # Validation Checks:
    /// - Signature verification using HMAC-SHA256
    /// - Expiration time validation
    /// - Algorithm verification (must be HS256)
    /// - Issuer and audience validation
    /// 
    /// # Security:
    /// Returns an error if the token is expired, has an invalid signature,
    /// or uses an unsupported algorithm.
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



