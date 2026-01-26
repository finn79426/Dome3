# Dome3: Your Web3 Endpoint Protection Tool

<p align="center">
  <img src="https://img.shields.io/badge/-Rust-black?style=flat-square&logo=rust&logoColor=white" alt="Rust">
  <img src="https://custom-icon-badges.demolab.com/badge/Windows-0078D6?style=flat-square&logo=windows11&logoColor=white" alt="Windows">
  <img src="https://img.shields.io/badge/-macOS-black?style=flat-square&logo=apple&logoColor=white" alt="macOS">
</p>

A background daemon that automatically detects, validates, and displays wallet details whenever you copy an address.

No more Chrome extension installations!

## Features

- Real-time Monitoring: Instant detection of wallet addresses on your clipboard.
- Threat Intelligence Integration: Powered by trustworthy, open-source blockchain threat data (e.g., Revoke.cash, ScamSniffer, Scorechain).
- Custom Labeling: Easily manage and edit custom labels for known wallet addresses.
- Focus Mode: "Do Not Disturb" support for uninterrupted workflow. (convenient for a trader, investigator)

## FAQ

### Does monitoring the OS clipboard compromise my privacy?

**No. Dome3 follows a "Privacy-First" design philosophy.**

The application is designed to react only to strings that match specific blockchain address patterns.

- Zero Logging: We do not store, upload, or analyze any clipboard content that isn't a wallet address.
- Open Source: The application is open-source. You are welcome to review the [clipboard source code](/src/clipboard.rs) to verify our handling logic.

### How do I add an external blockchain intelligence source?

**Dome3 is designed to be extensible.** If you wish to integrate third-party providers (e.g., Chainalysis, MistTrack), follow these steps:

1. Create a new module under externals/ to handle the API logic.
2. Ensure your struct implements the `externals::mod::Evaluation` trait.
3. Implement the evaluate function for your struct.
4. Register your new source by adding `<YourAPIProvider>::evaluate()` to the `external::mod::evaluate_all` function.

If you require such integration, **we strongly recommend opening a PR ticket** to collaborate.

We're welcome to incorporate any useful blockchain intelligence sources to make the Web3 ecosystem safer!

### How to import/export existing labeled addresses list?

For portability and batch editing, **the labeled address list is deliberately designed as a simple CSV format**. You can directly edit this file to import or export data.

CSV Location:

- macOS: `Dome3.app/labeled_address.csv`
- Windows: The same directory as `Dome3.exe`

CSV Format Example:

```csv
network,address,label
Bitcoin,bc1prsxhzhrg32symn8367xuerfrcrs6ensvjnmauphpm4zt0nr6rpnqnlzv6g,Alice
EVM,0x834481D85Ab097B26d10560c0DF89BCE7718dB88,Bob
Tron,TYA6CdvraMCzjokGA28koxdfiWgBjF4CQc,Charlie
Solana,BbtgEFyNvVRnmQBYhgCPNcv6BAe99xjQFQkgSfB9Wu7D,Diana
Polkadot,5Evad6C4kvs1TMkYzgRjtGtYfF5Qw4jrgkfYPP45cS5MT3dp,Fred
```
