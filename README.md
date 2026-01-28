<div align="center">

![demo](/gallery/Demo.png)

<p align="center">
  <img src="https://img.shields.io/badge/-Rust-black?style=flat-square&logo=rust&logoColor=white" alt="Rust">
  <img src="https://custom-icon-badges.demolab.com/badge/Windows-0078D6?style=flat-square&logo=windows11&logoColor=white" alt="Windows">
  <img src="https://img.shields.io/badge/-macOS-black?style=flat-square&logo=apple&logoColor=white" alt="macOS">
</p>

# Dome3

An antivirus-like endpoint protection tool for crypto natives.

</div>

## ✨ Features

### Eliminate risks at the earliest stage

We believe system clipboard is your first line of defense against hacks, phishing and scams.

Dome3 monitors your system clipboard, reacts when you are trying to copy a wallet address.

<https://github.com/user-attachments/assets/28a8c7a3-85ec-4dca-ba59-ec4ad2aa5738>

It's threat intel is powered trustworthy security vendors, and open-source dataset.

If you'd like to, you can also edit your own allow list.

<https://github.com/user-attachments/assets/5675b504-6ad2-4d5d-894c-1d97e979ae3d>

## Getting Started

Download the latest application binary from [Releases](https://github.com/finn79426/Dome3/releases).

Double click to open the application.

Done!

Whenever you copy an wallet address, a notification message will be pop-up.

## FAQ

### Does monitoring the OS clipboard compromise my privacy?

**No. Dome3 follows a "Privacy-First" design philosophy.**

The application is designed to react only to strings that match specific blockchain address patterns.

- Zero Logging: We do not store, upload, or analyze any clipboard content that isn't a wallet address.
- Open Source: The application is open-source. You are welcome to review the [clipboard source code](/src/clipboard.rs) to verify handling logic.

### How do I add an external blockchain intelligence source?

**Dome3 is designed to be extensible.**

If you like to introduce paid intel providers (e.g., Chainalysis, MistTrack), follow these steps:

1. Create a new module under externals/ to handle the API logic.
2. Ensure your struct implements the `externals::mod::Evaluation` trait.
3. Implement the evaluate function for your struct.
4. Register your new source by adding `<YourAPIProvider>::evaluate()` to the `external::mod::evaluate_all` function.

However, if you require such integration, **we strongly recommend opening a PR ticket** to collaborate.

We're welcome to incorporate any useful blockchain intelligence sources to make the Web3 ecosystem safer!

### How to import/export existing labeled addresses list?

For data portability and batch editing, **the labeled address list is deliberately designed as a simple CSV file**. You can directly edit this file to import or export data.

CSV File Path:

- macOS: `~/Library/Application Support/com.dome3.app/labeled_addresses.csv`
- Windows: `%APPDATA%\dome3\app\data\labeled_addresses.csv`

The CSV format is designed as simply as possible.

As a reference, CSV file should looks something like this:

```csv
network,address,label
Bitcoin,bc1prsxhzhrg32symn8367xuerfrcrs6ensvjnmauphpm4zt0nr6rpnqnlzv6g,Alice
EVM,0x834481D85Ab097B26d10560c0DF89BCE7718dB88,Bob
Tron,TYA6CdvraMCzjokGA28koxdfiWgBjF4CQc,Charlie
Solana,BbtgEFyNvVRnmQBYhgCPNcv6BAe99xjQFQkgSfB9Wu7D,Diana
Polkadot,5Evad6C4kvs1TMkYzgRjtGtYfF5Qw4jrgkfYPP45cS5MT3dp,Fred
```

### How to kill/restart the process?

You can use Task Manager (on Windows) or Activity Monitor (on macOS) to find the Dome3 process, then terminate it manually.

