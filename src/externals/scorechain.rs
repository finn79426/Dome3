use crate::crypto::NetworkRecognition;
use crate::externals::Evaluation;
use crate::models::{AddressLabel, AdvisoryLevel};
use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use log::info;
use reqwest;
use serde_json;
use std::time::Duration;
use std::time::Instant;

pub struct Dependency {
    pub http_client: reqwest::Client,
}

pub struct Scorechain {
    dependency: Dependency,
    api_key: String,
    cached_response: DashMap<String, (serde_json::Value, Instant)>,
}

#[async_trait]
impl Evaluation for Scorechain {
    async fn evaluate(&self, address: &str) -> Result<(AdvisoryLevel, AddressLabel)> {
        let is_sanctioned = self.is_sanctioned(address).await;

        if is_sanctioned {
            let name = self
                .name(address)
                .await
                .unwrap_or("OFAC Sanctioned Entity".to_string());

            return Ok((
                AdvisoryLevel::Danger,
                AddressLabel {
                    network: address.guess_network(),
                    address: address.to_string(),
                    label: name,
                },
            ));
        }

        Ok((AdvisoryLevel::Warning, AddressLabel::from(address)))
    }
}

impl Scorechain {
    pub fn new(dep: Dependency) -> Self {
        Self {
            dependency: dep,
            api_key: "4117dd88-9dcc-4755-91d4-6510f1bea6a7".to_string(), // It's a free API key, I don't care about leakage 😏
            cached_response: DashMap::new(),
        }
    }

    async fn is_sanctioned(&self, address: &str) -> bool {
        let response_json = self.fetch_or_cache(address).await;

        response_json
            .pointer("/isSanctioned")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    async fn name(&self, address: &str) -> Option<String> {
        let response_json = self.fetch_or_cache(address).await;

        response_json
            .pointer("/details/name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    async fn fetch_or_cache(&self, address: &str) -> serde_json::Value {
        const TTL: Duration = Duration::from_secs(24 * 60 * 60);

        if let Some(entry) = self.cached_response.get(address) {
            if entry.1.elapsed() < TTL {
                info!("🗂️ Cache Hit (Valid): {}", address);
                return entry.value().0.clone();
            } else {
                info!("♻️ Cache Hit (Expired): {}", address);
            }
        }

        info!("⌛️ Fetching Scorechain Sanctions API: {}", address);
        let url = format!("https://sanctions.api.scorechain.com/v1/addresses/{address}");
        let response = self
            .dependency
            .http_client
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .unwrap();
        let response_json: serde_json::Value = response.json().await.unwrap();

        self.cached_response
            .insert(address.to_string(), (response_json.clone(), Instant::now()));

        response_json
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest;

    #[tokio::test]
    #[ignore = "Will send HTTP requests to Scorechain API"]
    async fn test_evaluate() {
        let dep = Dependency {
            http_client: reqwest::Client::new(),
        };
        let this = Scorechain::new(dep);
        let address = "0x19Aa5Fe80D33a56D56c78e82eA5E50E5d80b4Dff";
        let result = this.evaluate(address).await;
        assert!(result.is_ok());
        assert!(result.as_ref().unwrap().0 == AdvisoryLevel::Danger);
        assert!(
            result.as_ref().unwrap().1.label
                == "SUEX OTC, S.R.O. - Successful Exchange (OFAC)".to_string()
        );
    }

    #[tokio::test]
    #[ignore = "Will send HTTP requests to Scorechain API"]
    async fn test_is_sanctioned() {
        let dep = Dependency {
            http_client: reqwest::Client::new(),
        };
        let this = Scorechain::new(dep);
        let address = "0x19Aa5Fe80D33a56D56c78e82eA5E50E5d80b4Dff";
        assert!(this.is_sanctioned(address).await == true);
    }

    #[tokio::test]
    #[ignore = "Will send HTTP requests to Scorechain API"]
    async fn test_name() {
        let dep = Dependency {
            http_client: reqwest::Client::new(),
        };
        let this = Scorechain::new(dep);
        let address = "0x19Aa5Fe80D33a56D56c78e82eA5E50E5d80b4Dff";
        assert!(
            this.name(address).await
                == Some("SUEX OTC, S.R.O. - Successful Exchange (OFAC)".to_string())
        );
    }
}
