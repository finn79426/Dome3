use crate::crypto::NetworkRecognition;
use crate::externals::Evaluation;
use crate::models::{AddressLabel, AdvisoryLevel};
use anyhow::bail;
use anyhow::{Context, Result};
use async_trait::async_trait;
use git2::build::RepoBuilder;
use git2::{Direction, FetchOptions, ObjectType, Remote, Repository, ResetType};
use log::{error, info, warn};
use serde_json;
use std::env;
use std::fs;
use std::path::PathBuf;
use tokio;
use walkdir::WalkDir;

/// Data Source: https://github.com/RevokeCash/whois
/// Using Dataset: https://github.com/RevokeCash/whois/tree/main/data/manual/spenders

#[derive(Clone)]
pub struct RevokeCashWhois {
    remote_repo_url: String,
    local_repo_path: PathBuf,
}

#[async_trait]
impl Evaluation for RevokeCashWhois {
    async fn evaluate(&self, address: &str) -> Result<(AdvisoryLevel, AddressLabel)> {
        let this = self.clone();
        let address_owned = address.to_string();

        let result = tokio::task::spawn_blocking(move || -> Result<(AdvisoryLevel, AddressLabel)> {
            let latest_local_commit_id = this.latest_commit(false)?;
            let latest_remote_commit_id = this.latest_commit(true)?;

            if latest_local_commit_id != latest_remote_commit_id {
                info!("🔄 Local repository is outdated, updating...");
                this.clone_or_pull()?;
            }

            if let Some(label) = this.is_exploit_spender(&address_owned) {
                return Ok((
                    AdvisoryLevel::Danger,
                    AddressLabel {
                        network: address_owned.guess_network(),
                        address: address_owned,
                        label: format!("Known Malicious Drainer ({label}) - Revoke approvals immediately!!!"),
                    },
                ));
            }

            Ok((AdvisoryLevel::Warning, AddressLabel::from(address_owned.as_str())))

        }).await.context("Failed to spawn blocking task")??;

        Ok(result)
    }
}

impl RevokeCashWhois {
    pub fn new() -> Self {
        Self {
            remote_repo_url: "https://github.com/RevokeCash/whois.git".to_string(),
            local_repo_path: env::temp_dir().join("RevokeCashWhois"),
        }
    }

    fn is_exploit_spender(&self, address: &str) -> Option<String> {
        let address_filename = format!("{}.json", address.to_canonical_address());
        let search_root = self.local_repo_path.join("data/manual/spenders");

        if !search_root.exists() {
            error!(
                "❌ Search root does not exist, please check that your repo was cloned correctly: {:?}",
                search_root
            );
            return None; // expect should never happen if everything is set up correctly
        }

        for entry in WalkDir::new(search_root).into_iter().filter_map(|r| r.ok()) {
            if entry.file_name() == address_filename.as_str() {
                let path = entry.path();

                let content = fs::read_to_string(path)
                    .with_context(|| format!("Failed to read file: {:?}", path))
                    .unwrap_or_default();

                let json: serde_json::Value = serde_json::from_str(&content)
                    .with_context(|| format!("Failed to parse JSON: {:?}", path))
                    .unwrap_or_default();

                let is_exploit = json
                    .get("riskFactors")
                    .and_then(|rf| rf.get(0))
                    .and_then(|item| item.get("type"))
                    .and_then(|t| t.as_str())
                    .map(|t| t == "exploit")
                    .unwrap_or(false);

                if is_exploit {
                    let label = json
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or("LABEL_NOT_FOUND")
                        .to_string();
                    return Some(label);
                }
            }
        }

        None
    }

    /// Get latest commit id
    fn latest_commit(&self, is_remote: bool) -> Result<String> {
        if is_remote {
            self.get_remote_latest_commit()
        } else {
            Ok(self.get_local_latest_commit().unwrap_or_default())
        }
    }

    /// Clone the repository if it does not exist, otherwise pull updates
    fn clone_or_pull(&self) -> Result<()> {
        if self.local_repo_path.exists() {
            match Repository::open(self.local_repo_path.as_os_str()) {
                Ok(_repo) => {
                    info!("🔄 Pulling updates...");
                    self.pull_repo()
                }
                Err(_) => {
                    warn!("🪬 Directory exist but repo seems broken!");
                    warn!("🚮 Removing broken repo directory...");
                    fs::remove_dir_all(&self.local_repo_path)
                        .context("Failed to remove broken repo directory")?;

                    info!("⌛️ Re-cloning repository...");
                    self.clone_repo()
                }
            }
        } else {
            info!("⌛️ Cloning repository...");
            self.clone_repo()
        }
    }

    /// Get latest commit id of the remote repository
    fn get_remote_latest_commit(&self) -> Result<String> {
        let mut remote = Remote::create_detached(self.remote_repo_url.as_str())
            .context("Failed to create detached remote")?;

        remote
            .connect(Direction::Fetch)
            .context("Failed to connect to remote")?;

        let refs = remote.list().context("Failed to list remote references")?;

        for head in refs {
            if head.name() == "refs/heads/main" {
                return Ok(head.oid().to_string());
            }
        }

        bail!("Failed to find 'refs/heads/main' in remote repository");
    }

    /// Get latest commit id of the local repository
    fn get_local_latest_commit(&self) -> Option<String> {
        if !self.local_repo_path.exists() {
            error!("❌ Local directory for the repository does not exist");
            return None;
        }

        let repo = match Repository::open(&self.local_repo_path) {
            Ok(r) => r,
            Err(_) => {
                error!("❌ Local directory exist but repo is broken (not a git repo)");
                return None;
            }
        };

        match repo.revparse_single("HEAD") {
            Ok(object) => Some(object.id().to_string()),
            Err(_) => {
                error!("❌ Local directory exist but repo is broken (HEAD not found)");
                None
            }
        }
    }

    /// Clone the remote repository to the local directory
    fn clone_repo(&self) -> Result<()> {
        info!(
            "⏬ Cloning repository to {}...",
            self.local_repo_path.to_string_lossy()
        );

        let mut fetch_options = FetchOptions::new();
        fetch_options.depth(1);

        let mut builder = RepoBuilder::new();
        builder.fetch_options(fetch_options);

        builder
            .clone(self.remote_repo_url.as_str(), &self.local_repo_path)
            .context("Failed to clone repository")?;

        info!("✅ Repo cloning finished successfully.");
        Ok(())
    }

    /// Pull updates from the remote repository
    fn pull_repo(&self) -> Result<()> {
        let repo =
            Repository::open(&self.local_repo_path).context("Failed to open local repository")?;

        let mut remote = repo
            .find_remote("origin")
            .context("Failed to find 'origin' remote")?;

        let mut fetch_options = FetchOptions::new();
        fetch_options.depth(1);

        remote
            .fetch(&["main"], Some(&mut fetch_options), None)
            .context("Failed to fetch from remote")?;

        let fetch_head = repo
            .find_reference("FETCH_HEAD")
            .context("Failed to find FETCH_HEAD")?;

        let fetch_commit = fetch_head
            .peel(ObjectType::Commit)
            .context("Failed to peel FETCH_HEAD to commit")?;

        repo.reset(&fetch_commit, ResetType::Hard, None)
            .context("Failed to perform hard reset")?;

        info!("✅ Repo pulling finished successfully.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Will send a real HTTP request to GitHub, which may trigger their anti-crawler measures"]
    async fn test_evaluate() {
        let this = RevokeCashWhois::new();
        let address = "0x5a0aB5d78C4d40E3a467a8BC52cE16Cce88c999D";
        let result = this.evaluate(address).await;
        assert!(result.is_ok());
        assert!(result.as_ref().unwrap().0 == AdvisoryLevel::Danger);
        assert!(
            result
                .as_ref()
                .unwrap()
                .1
                .label
                .starts_with("Known Malicious Drainer")
        );
    }
}
