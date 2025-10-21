//! # Card Validation Module
//! 
//! This module provides comprehensive card validation and fraud detection services
//! for the Payment Processor. It implements industry-standard validation algorithms
//! and security checks to ensure payment card integrity and prevent fraud.
//! 
//! ## Key Features:
//! - Luhn algorithm for card number validation
//! - Expiry date validation
//! - CVV validation based on card type
//! - Billing address validation
//! - Card type detection (Visa, Mastercard, Amex, Discover)
//! - Card number masking for security
//! - Fraud detection with pattern matching
//! - Blocked card management
//! 
//! ## Security Features:
//! - PCI DSS compliant card handling
//! - Card number masking for logs and display
//! - Fraud pattern detection
//! - Blocked card database
//! - User verification checks

use shared::{CardInfo, PaymentError, BillingAddress};
use std::collections::HashMap;
use chrono::Datelike;

/// Card validation utilities providing comprehensive card validation services
/// 
/// This struct contains static methods for validating payment cards using
/// industry-standard algorithms and security checks. All methods are stateless
/// and can be called concurrently.
pub struct CardValidator;

impl CardValidator {
    /// Validates a card number using the Luhn algorithm
    /// 
    /// The Luhn algorithm (also known as the "modulus 10" algorithm) is a simple
    /// checksum formula used to validate a variety of identification numbers,
    /// most notably credit card numbers. It detects any single-digit error and
    /// most adjacent digit transposition errors.
    /// 
    /// # Algorithm:
    /// 1. Remove all non-digit characters from the PAN
    /// 2. Check that the length is between 13-19 digits
    /// 3. Starting from the rightmost digit, double every second digit
    /// 4. If doubling results in a two-digit number, add the digits together
    /// 5. Sum all the digits
    /// 6. If the sum is divisible by 10, the card number is valid
    /// 
    /// # Parameters:
    /// - `pan`: Primary Account Number (card number) to validate
    /// 
    /// # Returns:
    /// - `Ok(())`: Card number is valid
    /// - `Err(PaymentError::InvalidCard)`: Card number is invalid
    /// 
    /// # Supported Lengths:
    /// - Visa: 13-19 digits
    /// - Mastercard: 16 digits
    /// - American Express: 15 digits
    /// - Discover: 16 digits
    /// 
    /// # Example:
    /// ```rust
    /// assert!(CardValidator::validate_card_number("4242424242424242").is_ok());
    /// assert!(CardValidator::validate_card_number("4242424242424243").is_err());
    /// ```
    pub fn validate_card_number(pan: &str) -> Result<(), PaymentError> {
        // Remove spaces and non-digits
        let cleaned_pan: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
        
        if cleaned_pan.len() < 13 || cleaned_pan.len() > 19 {
            return Err(PaymentError::InvalidCard("Invalid card number length".to_string()));
        }

        // Luhn algorithm
        let mut sum = 0;
        let mut double = false;
        
        for c in cleaned_pan.chars().rev() {
            if let Some(mut digit) = c.to_digit(10) {
                if double {
                    digit *= 2;
                    if digit > 9 {
                        digit = digit / 10 + digit % 10;
                    }
                }
                sum += digit;
                double = !double;
            }
        }

        if sum % 10 != 0 {
            return Err(PaymentError::InvalidCard("Invalid card number (Luhn check failed)".to_string()));
        }

        Ok(())
    }

    /// Validates card expiry date to ensure the card is not expired
    /// 
    /// This function checks that the card's expiry date is valid and that the
    /// card has not expired. It validates the month range and compares against
    /// the current date to ensure the card is still valid for use.
    /// 
    /// # Parameters:
    /// - `month`: Expiry month (1-12)
    /// - `year`: Expiry year (4-digit year)
    /// 
    /// # Returns:
    /// - `Ok(())`: Expiry date is valid and card is not expired
    /// - `Err(PaymentError::InvalidCard)`: Invalid month or card has expired
    /// 
    /// # Validation Rules:
    /// - Month must be between 1 and 12
    /// - Year must be current year or later
    /// - If year is current year, month must be current month or later
    /// 
    /// # Example:
    /// ```rust
    /// // Valid future date
    /// assert!(CardValidator::validate_expiry(12, 2025).is_ok());
    /// 
    /// // Expired card
    /// assert!(CardValidator::validate_expiry(1, 2020).is_err());
    /// 
    /// // Invalid month
    /// assert!(CardValidator::validate_expiry(13, 2025).is_err());
    /// ```
    pub fn validate_expiry(month: u8, year: u16) -> Result<(), PaymentError> {
        if month < 1 || month > 12 {
            return Err(PaymentError::InvalidCard("Invalid expiry month".to_string()));
        }

        let current_year = chrono::Utc::now().year() as u16;
        let current_month = chrono::Utc::now().month() as u8;

        if year < current_year || (year == current_year && month < current_month) {
            return Err(PaymentError::InvalidCard("Card has expired".to_string()));
        }

        Ok(())
    }

    /// Validates CVV based on card type
    pub fn validate_cvv(cvv: &str, pan: &str) -> Result<(), PaymentError> {
        let cleaned_pan: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
        
        if cleaned_pan.len() < 13 {
            return Err(PaymentError::InvalidCard("Invalid card number for CVV validation".to_string()));
        }

        // Determine card type by first digit
        let first_digit = cleaned_pan.chars().next().unwrap();
        let expected_cvv_length = match first_digit {
            '3' => 4, // American Express
            '4' | '5' | '6' => 3, // Visa, Mastercard, Discover
            _ => 3,
        };

        if cvv.len() != expected_cvv_length || !cvv.chars().all(|c| c.is_ascii_digit()) {
            return Err(PaymentError::InvalidCard(format!(
                "Invalid CVV length for card type (expected {})", 
                expected_cvv_length
            )));
        }

        Ok(())
    }

    /// Validates billing address
    pub fn validate_billing_address(address: &BillingAddress) -> Result<(), PaymentError> {
        if address.line1.trim().is_empty() {
            return Err(PaymentError::InvalidCard("Billing address line1 is required".to_string()));
        }

        if address.city.trim().is_empty() {
            return Err(PaymentError::InvalidCard("City is required".to_string()));
        }

        if address.postal_code.trim().is_empty() {
            return Err(PaymentError::InvalidCard("Postal code is required".to_string()));
        }

        if address.country.trim().is_empty() {
            return Err(PaymentError::InvalidCard("Country is required".to_string()));
        }

        // Basic postal code validation
        if address.postal_code.len() < 3 {
            return Err(PaymentError::InvalidCard("Invalid postal code".to_string()));
        }

        Ok(())
    }

    /// Comprehensive card validation
    pub fn validate_card(card: &CardInfo) -> Result<(), PaymentError> {
        Self::validate_card_number(&card.pan)?;
        Self::validate_expiry(card.expiry_month, card.expiry_year)?;
        Self::validate_cvv(&card.cvv, &card.pan)?;
        Self::validate_billing_address(&card.billing_address)?;

        // Additional validations
        if card.cardholder_name.trim().is_empty() {
            return Err(PaymentError::InvalidCard("Cardholder name is required".to_string()));
        }

        Ok(())
    }

    /// Detects card type from PAN
    pub fn get_card_type(pan: &str) -> String {
        let cleaned_pan: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
        
        if cleaned_pan.starts_with("4") {
            "Visa".to_string()
        } else if cleaned_pan.starts_with("5") && (cleaned_pan.chars().nth(1).unwrap() >= '1' && cleaned_pan.chars().nth(1).unwrap() <= '5') {
            "Mastercard".to_string()
        } else if cleaned_pan.starts_with("3") && (cleaned_pan.chars().nth(1).unwrap() == '4' || cleaned_pan.chars().nth(1).unwrap() == '7') {
            "American Express".to_string()
        } else if cleaned_pan.starts_with("6") {
            "Discover".to_string()
        } else {
            "Unknown".to_string()
        }
    }

    /// Masks card number for display
    pub fn mask_card_number(pan: &str) -> String {
        let cleaned_pan: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
        
        if cleaned_pan.len() < 4 {
            return "*".repeat(cleaned_pan.len());
        }

        let last_four = &cleaned_pan[cleaned_pan.len() - 4..];
        let masked = "*".repeat(cleaned_pan.len() - 4);
        format!("{}{}", masked, last_four)
    }
}

/// Fraud detection utilities
pub struct FraudDetector {
    // In a real system, this would connect to fraud detection services
    blocked_cards: HashMap<String, String>,
    suspicious_patterns: Vec<String>,
}

impl FraudDetector {
    pub fn new() -> Self {
        Self {
            blocked_cards: HashMap::new(),
            suspicious_patterns: vec![
                "0000000000000000".to_string(),
                "1111111111111111".to_string(),
                "1234567890123456".to_string(),
            ],
        }
    }

    /// Checks for fraud indicators
    pub fn check_fraud(&self, card: &CardInfo, user_info: &Option<shared::UserInfo>) -> Result<(), PaymentError> {
        // Check for known test/blocked cards
        if self.blocked_cards.contains_key(&card.pan) {
            return Err(PaymentError::FraudDetected("Card is blocked".to_string()));
        }

        // Check for suspicious patterns
        if self.suspicious_patterns.contains(&card.pan) {
            return Err(PaymentError::FraudDetected("Suspicious card pattern detected".to_string()));
        }

        // Check for repeated use (in a real system, this would check a database)
        // For now, we'll just do basic validation

        // Check if user info is provided and valid
        if let Some(user) = user_info {
            if !user.is_verified {
                return Err(PaymentError::FraudDetected("User not verified".to_string()));
            }
        }

        Ok(())
    }

    /// Adds a card to the blocked list (for testing)
    pub fn block_card(&mut self, pan: &str, reason: &str) {
        self.blocked_cards.insert(pan.to_string(), reason.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_card_number() {
        // Valid Visa card
        assert!(CardValidator::validate_card_number("4242424242424242").is_ok());
        
        // Invalid card (wrong checksum)
        assert!(CardValidator::validate_card_number("4242424242424243").is_err());
        
        // Invalid length
        assert!(CardValidator::validate_card_number("1234567890").is_err());
    }

    #[test]
    fn test_validate_expiry() {
        let current_year = chrono::Utc::now().year() as u16;
        let current_month = chrono::Utc::now().month() as u8;
        
        // Valid future date
        assert!(CardValidator::validate_expiry(current_month + 1, current_year).is_ok());
        
        // Expired card
        assert!(CardValidator::validate_expiry(current_month - 1, current_year).is_err());
        
        // Invalid month
        assert!(CardValidator::validate_expiry(13, current_year + 1).is_err());
    }

    #[test]
    fn test_get_card_type() {
        assert_eq!(CardValidator::get_card_type("4242424242424242"), "Visa");
        assert_eq!(CardValidator::get_card_type("5555555555554444"), "Mastercard");
        assert_eq!(CardValidator::get_card_type("378282246310005"), "American Express");
    }

    #[test]
    fn test_mask_card_number() {
        assert_eq!(CardValidator::mask_card_number("4242424242424242"), "************4242");
        assert_eq!(CardValidator::mask_card_number("1234567890"), "******7890");
    }
}
