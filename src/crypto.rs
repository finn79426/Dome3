use crate::models::AddressFormat;
use bitcoin::Address as BitcoinAddress;
use bitcoin::Network as BitcoinNetwork;
use bs58;
use regex::Regex;
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use sp_core::crypto::AccountId32;
use sp_core::crypto::Ss58Codec;
use std::borrow::Cow;
use std::str::FromStr;
use std::sync::LazyLock;

static REGEX_P2PKH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^1[1-9A-HJ-NP-Za-km-z]{25,34}$").unwrap());

static REGEX_P2SH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^3[1-9A-HJ-NP-Za-km-z]{25,34}$").unwrap());

static REGEX_BECH32: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(bc1)[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{39}$|^(bc1)[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{59}$").unwrap()
});

static REGEX_ETH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^0x[0-9a-fA-F]{40}$").unwrap());

static REGEX_TRON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^T[1-9A-HJ-NP-Za-km-z]{33}$").unwrap());

static REGEX_SOLANA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[1-9A-HJ-NP-Za-km-z]{32,44}$").unwrap());

static REGEX_POLKADOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[1-9A-HJ-NP-Za-km-z]{47,48}$").unwrap());

pub trait NetworkRecognition {
    fn guess_network(&self) -> AddressFormat;
    fn to_canonical_address(&self) -> Cow<'_, str>;
    fn is_bitcoin(&self) -> bool;
    fn is_evm(&self) -> bool;
    fn is_tron(&self) -> bool;
    fn is_solana(&self) -> bool;
    fn is_polkadot(&self) -> bool;
}

impl NetworkRecognition for str {
    fn guess_network(&self) -> AddressFormat {
        match () {
            _ if self.is_bitcoin() => AddressFormat::Bitcoin,
            _ if self.is_evm() => AddressFormat::EVM,
            _ if self.is_tron() => AddressFormat::Tron,
            _ if self.is_solana() => AddressFormat::Solana,
            _ if self.is_polkadot() => AddressFormat::Polkadot,
            _ => AddressFormat::default(),
        }
    }

    fn to_canonical_address(&self) -> Cow<'_, str> {
        let network = self.guess_network();

        match network {
            AddressFormat::Bitcoin if self.starts_with("bc1") => {
                if self.chars().any(|c| c.is_uppercase()) {
                    Cow::Owned(self.to_lowercase())
                } else {
                    Cow::Borrowed(self)
                }
            }

            AddressFormat::EVM => {
                // Remove the "0x" prefix if present
                let addr = self.strip_prefix("0x").unwrap_or(self);

                // Remove the padded zeros if present
                // Eg. 0x000000000000000000000000dAC17F958D2ee523a2206206994597C13D831ec7 -> 0xdAC17F958D2ee523a2206206994597C13D831ec7
                let addr = if addr.len() > 40 {
                    let offset = addr.len() - 40;
                    &addr[offset..]
                } else {
                    addr
                };

                let addr_lower = addr.to_lowercase();
                let hash = Keccak256::digest(addr_lower.as_bytes());

                let mut checksum_addr = String::from("0x");

                for (i, c) in addr_lower.chars().enumerate() {
                    let hash_byte = hash[i / 2];
                    let hash_nibble = if i % 2 == 0 {
                        (hash_byte >> 4) & 0xF
                    } else {
                        hash_byte & 0xF
                    };
                    if c.is_digit(10) {
                        checksum_addr.push(c);
                    } else if hash_nibble >= 8 {
                        checksum_addr.push(c.to_ascii_uppercase());
                    } else {
                        checksum_addr.push(c);
                    }
                }

                debug_assert!(checksum_addr.starts_with("0x"));
                debug_assert!(checksum_addr.len() == 42);

                if self == checksum_addr {
                    Cow::Borrowed(self)
                } else {
                    Cow::Owned(checksum_addr)
                }
            }

            AddressFormat::Tron => {
                if self.starts_with('t') {
                    let mut fixed = self.to_string();
                    fixed.replace_range(0..1, "T");
                    Cow::Owned(fixed)
                } else {
                    Cow::Borrowed(self)
                }
            }

            _ => Cow::Borrowed(self),
        }
    }

    fn is_bitcoin(&self) -> bool {
        if !(REGEX_P2PKH.is_match(self) || REGEX_P2SH.is_match(self) || REGEX_BECH32.is_match(self))
        {
            return false;
        }

        match BitcoinAddress::from_str(self) {
            Ok(addr) => addr.require_network(BitcoinNetwork::Bitcoin).is_ok(),
            Err(_) => false,
        }
    }

    fn is_evm(&self) -> bool {
        let addr = self.strip_prefix("0x").unwrap_or(self);

        let addr = match addr.len() {
            len if len < 40 => return false,
            len if len > 40 => {
                // remove padded zeros
                let offset = len - 40;
                if addr[..offset].chars().all(|c| c == '0') {
                    format!("0x{}", &addr[offset..])
                } else {
                    return false;
                }
            }
            _ => format!("0x{}", addr),
        };

        debug_assert!(addr.len() == 42);
        debug_assert!(addr.starts_with("0x"));

        REGEX_ETH.is_match(&addr)
    }

    fn is_tron(&self) -> bool {
        if !REGEX_TRON.is_match(self) {
            return false;
        }

        debug_assert!(self.starts_with("T"));
        debug_assert!(self.len() == 34);

        let decoded = match bs58::decode(self).into_vec() {
            Ok(vec) => vec,
            Err(_) => return false,
        };

        if decoded.len() != 25 {
            return false;
        }

        let (body, checksum) = decoded.split_at(21);
        let hash = Sha256::digest(&Sha256::digest(body));
        let expected_checksum = &hash[..4];

        expected_checksum == checksum
    }

    fn is_solana(&self) -> bool {
        // IMPORTANT: PDA address also return true
        if !REGEX_SOLANA.is_match(self) {
            return false;
        }

        match bs58::decode(self).into_vec() {
            Ok(decoded) => decoded.len() == 32,
            Err(_) => false,
        }
    }

    fn is_polkadot(&self) -> bool {
        // IMPORTANT: Does not distinguish between Polkadot and Kusama; any SS58Check address will returns true.
        if !REGEX_POLKADOT.is_match(self) {
            return false;
        }

        AccountId32::from_ss58check(self).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_bitcoin_should_return_true() {
        assert!("164eTsjbZhCCubauBb4VLgkkFhnY1cE347".is_bitcoin()); // P2PKH
        assert!("39kz54D6ewchz3sXvncHjFYpcNGUrZ11Te".is_bitcoin()); // P2SH
        assert!("bc1qgll00eher0sferr6d5xsa9puxv8ez0z76xquyp".is_bitcoin()); // P2WPKH
        assert!("bc1qvhu3557twysq2ldn6dut6rmaj3qk04p60h9l79wk4lzgy0ca8mfsnffz65".is_bitcoin()); // P2WSH
        assert!("bc1p7gdx38p6n0xngzv4p8vjmu2e70ym0w9anwxxs7s6fpn7zjm0rwvsuugdey".is_bitcoin()); // P2TR
    }

    #[test]
    fn test_is_bitcoin_should_return_false() {
        assert!(!"1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfN9".is_bitcoin()); // P2PKH invalid checksum
        assert!(!"3J98t1WpEZ73CNmQviecrnyiWrnqRhWNL9".is_bitcoin()); // P2SH invalid checksum
        assert!(!"bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt081".is_bitcoin()); // P2WPKH invalid checksum
        assert!(!"bc1qrp33g0q5c5txsp9arysrx4k6zdkfs4n0a9muf4".is_bitcoin()); // P2WSH invalid checksum
        assert!(!"bc1p5cyxnuxmeuwuvkwfem96l0gdku6zkszt5v8a8h3u6d6c6r8s5w7qz6v7x0b".is_bitcoin()); // P2TR invalid checksum

        assert!(!"1DFGekrfqNNWGL7Gw7BW2pvYpZVRNmmg1".is_bitcoin()); // P2PKH too short
        assert!(!"39kz54D6ewchz3sXvncHjFYpcNGUrZ11T".is_bitcoin()); // P2SH too short
        assert!(!"bc1qgll00eher0sferr6d5xsa9puxv8ez0z76xquy".is_bitcoin()); // P2WPKH too short
        assert!(!"bc1qvhu3557twysq2ldn6dut6rmaj3qk04p60h9l79wk4lzgy0ca8mfsnffz6".is_bitcoin()); // P2WSH too short
        assert!(!"bc1p7gdx38p6n0xngzv4p8vjmu2e70ym0w9anwxxs7s6fpn7zjm0rwvsuugde".is_bitcoin()); // P2TR too short

        assert!(!"1DFGekrfqNNWGL7Gw7BW2pvYpZVRNmmg1O".is_bitcoin()); // P2PKH invalid char 'O'
        assert!(!"39kz54D6ewchz3sXvncHjFYpcNGUrZ11TeI".is_bitcoin()); // P2SH invalid char 'I'
        assert!(!"bc1qvhu3557twysq2ldn6dut6rmaj3qk04p60h9l79wk4lzgy0ca8mfsnffz6O".is_bitcoin()); // P2WSH invalid char 'O'
        assert!(!"bc1p7gdx38p6n0xngzv4p8vjmu2e70ym0w9anwxxs7s6fpn7zjm0rwvsuugdeO".is_bitcoin()); // P2TR invalid char 'O'

        assert!(!"".is_bitcoin()); // empty string
        assert!(!"hello world".is_bitcoin()); // text string
        assert!(!"1234567890".is_bitcoin()); // decimal string

        assert!(!"1notarealaddressatall".is_bitcoin()); // not a related address
        assert!(!"3notarealaddressatall".is_bitcoin()); // not a related address
        assert!(!"bc1qnotarealaddressatall".is_bitcoin()); // not a related address
        assert!(!"bc1pnotarealaddressatall".is_bitcoin()); // not a related address
    }

    #[test]
    fn test_is_evm_should_return_true() {
        assert!("0xdAC17F958D2ee523a2206206994597C13D831ec7".is_evm()); // checksum
        assert!("0xdac17f958d2ee523a2206206994597c13d831ec7".is_evm()); // all lower
        assert!("0xDAC17F958D2EE523A2206206994597C13D831EC7".is_evm()); // all upper
        assert!("0x000000000000000000000000dAC17F958D2ee523a2206206994597C13D831ec7".is_evm()); // full 32 bytes
        assert!("0x0000000000000000000000000000000000000000000000000000000000000000".is_evm()); // pre-compile address
        assert!("0x0000000000000000000000000000000000000000000000000000000000000001".is_evm()); // pre-compile address
        assert!("0x000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".is_evm()); // pre-compile address
    }

    #[test]
    fn test_is_evm_should_return_false() {
        assert!(!"hello world".is_evm()); // arbitrary string
        assert!(!"1234567890".is_evm()); // numeric string
        assert!(!"".is_evm()); // empty string
        assert!(!"0xnotarealaddressatall".is_evm()); // arbitrary string that matched 0x prefix
        assert!(!"0xdAC17F958D2ee523a2206206994597C13D831ecG".is_evm()); // invalid hex char G
        assert!(!"dAC17F958D2ee523a2206206994597C13D831ecG".is_evm()); // missing 0x
        assert!(!"0xdAC17F958D2ee523a2206206994597C13D831ec".is_evm()); // 41 chars
        assert!(!"0xfdAC17F958D2ee523a2206206994597C13D831ec7".is_evm()); // 43 chars (MSB not starts with '0')
    }

    #[test]
    fn test_is_tron_should_return_true() {
        assert!("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".is_tron());
    }

    #[test]
    fn test_is_tron_should_return_false() {
        assert!(!"hello world".is_tron()); // arbirary string
        assert!(!"1234567890".is_tron()); // numeric string
        assert!(!"".is_tron()); // empty string
        assert!(!"Tnotarealaddressatall".is_tron()); // arbirary string that matched T prefix
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj7t".is_tron()); // invalid checksum
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLjuu".is_tron()); // invalid last char
        assert!(!"SR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".is_tron()); // invalid prefix
        assert!(!"TR7NHqjeKQxGTCi8qZZZY4pL8otSzgjLj6t".is_tron()); // invalid middle char
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjL".is_tron()); // length too short
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6tt".is_tron()); // length too long
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pL0otSzgjLj6t".is_tron()); // invalid charset '0'
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pLOotSzgjLj6t".is_tron()); // invalid charset 'O'
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pLIotSzgjLj6t".is_tron()); // invalid charset 'I'
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pLlotSzgjLj6t".is_tron()); // invalid charset 'l'
    }

    #[test]
    fn test_is_solana_should_return_true() {
        assert!("6p6xgHy9S7B3D6DdeS9NAsAnC56p6D8Swn3M5rQJvXN2".is_solana());
        assert!("11111111111111111111111111111111".is_solana());
        assert!("HN7cABqLq46Es1sy9P2Af8uaYNLDajEzGHeLidXqumFc".is_solana());
    }

    #[test]
    fn test_is_solana_should_return_false() {
        assert!(!"6p6xgHy9S7B3D6DdeS9NAsAnC56p6D8Swn3M5rQJvXO1".is_solana());
        assert!(!"6p6xgHy9S7B3D6DdeS9NAsAnC56p6D8Swn3M5rQJvXN21".is_solana());
        assert!(!"ShortAddr".is_solana());
        assert!(!"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".is_solana());
    }

    #[test]
    fn test_is_polkadot_should_return_true() {
        assert!("1FRMM8PEiWXYax7rpS6X4XZX1aAAxSWx1CrKTyrVYhV24fg".is_polkadot());
        assert!("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".is_polkadot());
    }

    #[test]
    fn test_is_polkadot_should_return_false() {
        assert!(!"1FRMM8PEiWXYax7rpS6X4XZX1aAAxSWx1CrKTyrVYhV24fh".is_polkadot());
        assert!(!"5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQ".is_polkadot());
        assert!(!"6p6xgHy9S7B3D6DdeS9NAsAnC56p6D8Swn3M5rQJvXN2".is_polkadot());
        assert!(!"".is_polkadot());
        assert!(!"invalid_polkadot_address".is_polkadot());
    }
}
