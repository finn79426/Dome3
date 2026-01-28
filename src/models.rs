use crate::crypto::NetworkRecognition;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use strum_macros::EnumString;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AddressLabel {
    pub network: AddressFormat,
    pub address: String,
    pub label: String,
}

impl From<&str> for AddressLabel {
    fn from(address: &str) -> Self {
        Self {
            network: address.guess_network(),
            address: address.to_string(),
            label: "Unknown Wallet".to_string(),
        }
    }
}

#[derive(Default, Copy, Clone, Debug, Serialize, Deserialize, EnumString, PartialEq, Display)]
pub enum AddressFormat {
    Bitcoin,
    EVM,
    Tron,
    Solana,
    Polkadot,
    #[default]
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AdvisoryLevel {
    Unknown, // Waiting for querying result from 3rd-party APIs
    Known,   // Known wallet address - user takes their own risk
    Warning, // Low risk - basically no security concern, but user should stay vigilant
    Risky,   // Medium risk - detected some security concerns, not recommended to interact
    Danger,  // Severe risk - known malicious actor, DO NOT INTERACT
}
