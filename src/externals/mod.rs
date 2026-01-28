pub mod revokecash_exploit_approval_list;
pub mod revokecash_whois;
pub mod scamsniffer;
pub mod scorechain;
use crate::crypto::NetworkRecognition;
use crate::externals::revokecash_exploit_approval_list::RevokeCashApprovalExploitList;
use crate::externals::revokecash_whois::RevokeCashWhois;
use crate::externals::scamsniffer::ScamSniffer;
use crate::externals::scorechain::Scorechain;
use crate::models::{AddressLabel, AdvisoryLevel};
use anyhow::Result;
use anyhow::anyhow;
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use log::warn;
use reqwest;
use std::sync::LazyLock;
use std::time::Duration;

struct Evaluators {
    approval_exploit_list: RevokeCashApprovalExploitList,
    whois: RevokeCashWhois,
    scamsniffer: ScamSniffer,
    scorechain: Scorechain,
}

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .expect("Failed to build HTTP client")
});

static EVALUATORS: LazyLock<Evaluators> = LazyLock::new(|| Evaluators {
    approval_exploit_list: RevokeCashApprovalExploitList::new(),
    whois: RevokeCashWhois::new(),
    scamsniffer: ScamSniffer::new(scamsniffer::Dependency {
        http_client: HTTP_CLIENT.clone(),
    }),
    scorechain: Scorechain::new(scorechain::Dependency {
        http_client: HTTP_CLIENT.clone(),
    }),
});

#[async_trait]
pub trait Evaluation {
    async fn evaluate(&self, address: &str) -> Result<(AdvisoryLevel, AddressLabel)>;
}

pub async fn evaluate_all(address: &str) -> Result<(AdvisoryLevel, AddressLabel)> {
    let timeout = Duration::from_secs(10);

    let result = tokio::time::timeout(timeout, async {
        let mut tasks = FuturesUnordered::new();
        let mut cached_lowest_risk: Option<(AdvisoryLevel, AddressLabel)> = None;

        tasks.push(EVALUATORS.approval_exploit_list.evaluate(address));
        tasks.push(EVALUATORS.whois.evaluate(address));
        tasks.push(EVALUATORS.scamsniffer.evaluate(address));
        tasks.push(EVALUATORS.scorechain.evaluate(address));

        while let Some(result) = tasks.next().await {
            match result {
                Ok((level, label)) => match level {
                    AdvisoryLevel::Warning => {
                        cached_lowest_risk = Some((level, label));
                    }
                    _ => return Ok((level, label)),
                },
                Err(_) => continue,
            }
        }

        cached_lowest_risk.ok_or_else(|| anyhow!("Failed to evaluate address"))
    })
    .await;

    match result {
        Ok(result) => result,
        Err(_) => {
            warn!("🙁 Evaluation timed out, returning unknown result...");
            return Ok((
                AdvisoryLevel::Warning,
                AddressLabel {
                    network: address.guess_network(),
                    address: address.to_canonical_address().to_string(),
                    label: "Unknown Wallet".to_string(),
                },
            ));
        }
    }
}
