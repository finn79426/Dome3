use crate::crypto::NetworkRecognition;
use crate::externals::Evaluation;
use crate::models::{AddressLabel, AdvisoryLevel};
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use log::info;
use reqwest;
use serde_json;
use std::env;
use std::path::PathBuf;
use tokio;

pub struct Dependency {
    pub http_client: reqwest::Client,
}

pub struct ScamSniffer {
    dependency: Dependency,
    data_source_url: String,
    commits_history_url: String,
    latest_commit_id: String,
    data_filepath: PathBuf,
}

#[async_trait]
impl Evaluation for ScamSniffer {
    async fn evaluate(&self, address: &str) -> Result<(AdvisoryLevel, AddressLabel)> {
        if self.reload_is_needed().await {
            info!("🔄 Data is outdated, updating...");
            self.download().await?;
        }

        let content = tokio::fs::read_to_string(&self.data_filepath)
            .await
            .with_context(|| format!("Failed to open & read: {:?}", self.data_filepath))?;

        let json: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON: {:?}", self.data_filepath))?;

        let blacklist_addresses: Vec<String> = serde_json::from_value(json["address"].clone())
            .context("JSON missing 'address' field or format error")?;

        // 💡 Note: We expected that `all.json` to contain only the EVM address that has been converted to lowercase.
        // This might cause some matching issue if ScamSniffer updates formation.
        if blacklist_addresses.contains(&address.to_lowercase()) {
            return Ok((
                AdvisoryLevel::Danger,
                AddressLabel {
                    network: address.guess_network(),
                    address: address.to_string(),
                    label: "Known Scammer (Reported by ScamSniffer)".to_string(),
                },
            ));
        }

        Ok((AdvisoryLevel::Warning, AddressLabel::from(address)))
    }
}

impl ScamSniffer {
    pub fn new(dep: Dependency) -> Self {
        Self {
            dependency: dep,
            data_filepath: env::temp_dir().join("ScamSniffer.json"), // `all.json` will be downloaded & renamed to this
            data_source_url: "https://raw.githubusercontent.com/scamsniffer/scam-database/refs/heads/main/blacklist/all.json".to_string(),
            commits_history_url: "https://api.github.com/repos/scamsniffer/scam-database/commits?path=/blacklist/all.json&per_page=1".to_string(),
            latest_commit_id: String::new(),
        }
    }

    async fn reload_is_needed(&self) -> bool {
        (self.latest_commit_id != self.get_latest_commit().await) || !&self.data_filepath.exists()
    }

    async fn get_latest_commit(&self) -> String {
        let response = self
            .dependency
            .http_client
            .get(&self.commits_history_url)
            .send()
            .await
            .unwrap();

        let commits: Vec<serde_json::Value> = response.json().await.unwrap();

        commits
            .first()
            .expect("Unexpected result: commit list is empty")["sha"]
            .as_str()
            .expect("\"sha\" field missing")
            .to_string()
    }

    async fn download(&self) -> Result<()> {
        let response = self
            .dependency
            .http_client
            .get(&self.data_source_url)
            .send()
            .await
            .with_context(|| format!("Failed to send request: {}", self.data_source_url))?;

        let content = response
            .bytes()
            .await
            .with_context(|| format!("Failed to read response body: {}", self.data_source_url))?;

        if let Some(parent) = self.data_filepath.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await.with_context(|| {
                    format!(
                        "Failed to create parent directory: {:?}",
                        self.data_filepath
                    )
                })?;
            }
        }

        tokio::fs::write(&self.data_filepath, content)
            .await
            .with_context(|| format!("Failed to write file: {:?}", self.data_filepath))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest;

    #[tokio::test]
    #[ignore = "Will send HTTP requests to GitHub.com, which may trigger their anti-crawler measures"]
    async fn test_evaluate() {
        let dep = Dependency {
            http_client: reqwest::Client::new(),
        };
        let this = ScamSniffer::new(dep);
        let address = "0x379F983F9EdbA3a4b5055325467000f26DF6Cc43";
        let result = this.evaluate(address).await;

        assert!(result.is_ok());
        assert!(result.as_ref().unwrap().0 == AdvisoryLevel::Danger);
        assert!(
            result.as_ref().unwrap().1.label
                == "Known Scammer (Reported by ScamSniffer)".to_string()
        );
    }
}
