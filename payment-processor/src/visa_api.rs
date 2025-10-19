use anyhow::Result;
use serde::{Deserialize, Serialize};
use shared::{CardInfo, PaymentError};
use std::collections::HashMap;

/// Visa API client for processing payments
pub struct VisaApiClient {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VisaPaymentRequest {
    pub amount: String,
    pub currency: String,
    pub card_number: String,
    pub expiry_month: String,
    pub expiry_year: String,
    pub cvv: String,
    pub cardholder_name: String,
    pub billing_address: VisaBillingAddress,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VisaBillingAddress {
    pub line1: String,
    pub city: String,
    pub postal_code: String,
    pub country: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VisaPaymentResponse {
    pub status: String,
    pub transaction_id: String,
    pub auth_code: String,
    pub response_code: String,
    pub response_message: String,
    pub amount: String,
    pub currency: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VisaErrorResponse {
    pub error_code: String,
    pub error_message: String,
    pub details: Option<String>,
}

impl VisaApiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://sandbox.api.visa.com".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Process a payment through Visa API
    pub async fn process_payment(&self, card_info: &CardInfo, amount: i64, currency: &str) -> Result<VisaPaymentResponse, PaymentError> {
        // Convert card info to Visa API format
        let visa_request = VisaPaymentRequest {
            amount: format!("{:.2}", amount as f64 / 100.0), // Convert cents to dollars
            currency: currency.to_string(),
            card_number: card_info.pan.clone(),
            expiry_month: format!("{:02}", card_info.expiry_month),
            expiry_year: card_info.expiry_year.to_string(),
            cvv: card_info.cvv.clone(),
            cardholder_name: card_info.cardholder_name.clone(),
            billing_address: VisaBillingAddress {
                line1: card_info.billing_address.line1.clone(),
                city: card_info.billing_address.city.clone(),
                postal_code: card_info.billing_address.postal_code.clone(),
                country: card_info.billing_address.country.clone(),
            },
        };

        // For demo purposes, we'll simulate Visa API responses
        // In a real implementation, you would make actual HTTP calls to Visa API
        self.simulate_visa_api_call(&visa_request).await
    }

    /// Simulate Visa API call with demo responses
    async fn simulate_visa_api_call(&self, request: &VisaPaymentRequest) -> Result<VisaPaymentResponse, PaymentError> {
        // Simulate different responses based on card number patterns
        let card_number = &request.card_number;
        
        // Demo responses for different scenarios
        if card_number.starts_with("4000000000000002") {
            // Declined card
            return Err(PaymentError::InvalidCard("Card declined by issuer".to_string()));
        } else if card_number.starts_with("4000000000000119") {
            // Processing error
            return Err(PaymentError::InvalidCard("Processing error".to_string()));
        } else if card_number.starts_with("4000000000000259") {
            // Insufficient funds
            return Err(PaymentError::InsufficientFunds("Insufficient funds".to_string()));
        } else if card_number.starts_with("4242424242424242") {
            // Successful Visa card
            Ok(VisaPaymentResponse {
                status: "APPROVED".to_string(),
                transaction_id: format!("visa_{}", uuid::Uuid::new_v4()),
                auth_code: "AUTH123456".to_string(),
                response_code: "00".to_string(),
                response_message: "Transaction approved".to_string(),
                amount: request.amount.clone(),
                currency: request.currency.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        } else if card_number.starts_with("4") {
            // Generic Visa card - approved
            Ok(VisaPaymentResponse {
                status: "APPROVED".to_string(),
                transaction_id: format!("visa_{}", uuid::Uuid::new_v4()),
                auth_code: "AUTH789012".to_string(),
                response_code: "00".to_string(),
                response_message: "Transaction approved".to_string(),
                amount: request.amount.clone(),
                currency: request.currency.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        } else {
            // Unsupported card type
            Err(PaymentError::InvalidCard("Unsupported card type".to_string()))
        }
    }

    /// Validate card with Visa API
    pub async fn validate_card(&self, card_info: &CardInfo) -> Result<VisaPaymentResponse, PaymentError> {
        // For validation, we'll use a small amount
        self.process_payment(card_info, 100, "USD").await
    }

    /// Get card type information from Visa
    pub async fn get_card_type_info(&self, card_number: &str) -> Result<HashMap<String, String>, PaymentError> {
        let mut info = HashMap::new();
        
        if card_number.starts_with("4") {
            info.insert("card_type".to_string(), "Visa".to_string());
            info.insert("issuer".to_string(), "Visa Inc.".to_string());
            info.insert("country".to_string(), "US".to_string());
        } else if card_number.starts_with("5") {
            info.insert("card_type".to_string(), "Mastercard".to_string());
            info.insert("issuer".to_string(), "Mastercard Inc.".to_string());
            info.insert("country".to_string(), "US".to_string());
        } else if card_number.starts_with("3") {
            info.insert("card_type".to_string(), "American Express".to_string());
            info.insert("issuer".to_string(), "American Express".to_string());
            info.insert("country".to_string(), "US".to_string());
        } else {
            return Err(PaymentError::InvalidCard("Unknown card type".to_string()));
        }

        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_visa_api_successful_payment() {
        let client = VisaApiClient::new("test_key".to_string());
        
        let card_info = CardInfo {
            pan: "4242424242424242".to_string(),
            expiry_month: 12,
            expiry_year: 2025,
            cvv: "123".to_string(),
            cardholder_name: "John Doe".to_string(),
            billing_address: shared::BillingAddress {
                line1: "123 Main St".to_string(),
                city: "New York".to_string(),
                postal_code: "10001".to_string(),
                country: "US".to_string(),
            },
        };

        let result = client.process_payment(&card_info, 10000, "USD").await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.status, "APPROVED");
        assert_eq!(response.currency, "USD");
        assert_eq!(response.amount, "100.00");
    }

    #[tokio::test]
    async fn test_visa_api_declined_payment() {
        let client = VisaApiClient::new("test_key".to_string());
        
        let card_info = CardInfo {
            pan: "4000000000000002".to_string(),
            expiry_month: 12,
            expiry_year: 2025,
            cvv: "123".to_string(),
            cardholder_name: "John Doe".to_string(),
            billing_address: shared::BillingAddress {
                line1: "123 Main St".to_string(),
                city: "New York".to_string(),
                postal_code: "10001".to_string(),
                country: "US".to_string(),
            },
        };

        let result = client.process_payment(&card_info, 10000, "USD").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_visa_api_card_type_info() {
        let client = VisaApiClient::new("test_key".to_string());
        
        let result = client.get_card_type_info("4242424242424242").await;
        assert!(result.is_ok());
        
        let info = result.unwrap();
        assert_eq!(info.get("card_type").unwrap(), "Visa");
        assert_eq!(info.get("issuer").unwrap(), "Visa Inc.");
    }
}
