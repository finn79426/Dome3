use crate::crypto::NetworkRecognition;
use crate::csv;
use crate::externals::evaluate_all;
use crate::models::AddressFormat;
use crate::models::{AddressLabel, AdvisoryLevel};
use arboard::Clipboard;
use log::info;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

const MAX_WALLET_ADDRESS_LENGTH: usize = 70;

pub fn start_listening(
    csv_context: Arc<Mutex<csv::Context>>,
    tx: mpsc::UnboundedSender<(AdvisoryLevel, AddressLabel)>,
) {
    let runtime = Runtime::new().expect("Failed to new tokio::runtime::Runtime");
    let mut clipboard = Clipboard::new().expect("Failed to new arboard::Clipboard");
    let mut prev_content = String::new();

    loop {
        thread::sleep(Duration::from_millis(50));

        if tx.is_closed() {
            break;
        }

        if let Ok(content) = clipboard.get_text() {
            if content.len() > MAX_WALLET_ADDRESS_LENGTH
                || content == prev_content
                || content.guess_network() == AddressFormat::default()
            {
                continue;
            } else {
                prev_content = content.clone();
            }

            let network = content.guess_network();
            let address = content.to_canonical_address();
            // ⚠️ Pls note that`address` may not be a eligible wallet address even we called `to_canonical_address`.
            //    We only standardized its string format for following string comparison operations.

            if let Some(address_label) = csv_context.lock().unwrap().find(&network, &address) {
                info!("👀 Found existing label in CSV: {:?}", address_label);
                let _ = tx.send((AdvisoryLevel::Known, address_label.clone()));
            } else {
                let address = address.to_string();

                let _ = tx.send((
                    AdvisoryLevel::Unknown,
                    AddressLabel {
                        network,
                        address: address.clone(),
                        label: "🔍 Checking Label...".to_string(),
                    },
                ));

                let tx = tx.clone();

                runtime.spawn(async move {
                    if let Ok((advisory_level, address_label)) = evaluate_all(&address).await {
                        info!(
                            "🤖 Found address label: {:?} with level: {:?} from external APIs",
                            address_label, advisory_level
                        );
                        let _ = tx.send((advisory_level, address_label));
                    }
                });
            }
        }
    }
}
