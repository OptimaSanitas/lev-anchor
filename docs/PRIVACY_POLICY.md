# SanitasSeeker Privacy Policy

**Last Updated: April 26, 2026**

## Introduction

**Optima Sanitas** ("we", "us", or "our") is committed to protecting your privacy. This Privacy Policy explains how we collect, use, disclose, and safeguard your information when you use **SanitasSeeker** (the "App") on **[Solana Mobile Seeker](https://solanamobile.com/seeker)**—Solana Mobile’s **Android** phone built for the Solana ecosystem. The current release is **Android-only**; we do not ship an iOS build. The App provides sprint interval training, optional on-chain SKE-Claim pool rewards (SKR) when enabled—not guaranteed or on a fixed daily schedule, daily longevity/news threads, and related features.

By using the App, you consent to the data practices described in this policy.

## Information We Collect

### 1. Personal & Fitness Data
- **Exercise Metrics**: Distance walked/ran (GPS), sets completed, speed, accelerometer data (for movement detection fusion), calories burned (estimated).
- **Workout History**: Timestamps, phase durations, total progress stored locally and logged on-chain via `log_workout` and `update_fitness_stats` instructions.
- **SBT Data**: Soulbound Token metadata (labels, version, is_early/Legend status) stored on-chain.

### 2. Location Data (GPS)
- Precise location (latitude/longitude) is collected **only during active workout sessions** for accurate distance tracking in sprint intervals.
- Location permission is requested at runtime. We do **not** track location in the background or when the App is closed.
- Data is processed locally on-device for real-time speed/distance and sent on-chain only as aggregated workout summaries.

### 3. Wallet & On-Chain Data
- Solana wallet address and public key (via Mobile Wallet Adapter / Seed Vault).
- Transaction signatures for daily micro-rewards (`claim_daily_reward`), SBT minting (`mint_sbt`), and SKR claims.
- On-chain account data (UserExerciseState, SbtAccount, etc.) is publicly visible on Solana explorers.

### 4. Device & Usage Data
- Device model (**Solana Mobile Seeker** / Android), OS version, app version.
- App usage analytics (screens visited, workout completion rates) — stored locally only.
- No cookies or cross-app tracking.

### 5. News & Social Data
- Daily issue content (workout video URLs, text, optional persona image, optional outbound links) resolved from the on-chain PDA (`daily-news-seeker-final`) and static URLs you configure (e.g. Arweave). No X/Twitter account data is collected in the app.
- No personal X account data is collected or stored.

## How We Use Your Information

- **Core Functionality**: Enable accurate sprint interval training, GPS distance tracking, and real-time phase transitions (walk → run → walk).
- **On-Chain Rewards**: Verify Seeker Genesis NFT ownership, log workouts, and, when the admin enables claims and the pool is funded, distribute SKR from the SKE-Claim pool (program pool; no promise of a daily amount or schedule).
- **SBT Minting & Legend System**: Create and update personal Soulbound Tokens with your fitness achievements and optional paid Legend extensions.
- **News Feed**: Deliver curated longevity content directly from the on-chain daily news PDA.
- **Improvement**: Analyze aggregated (anonymized) workout trends to improve interval recommendations (future LEVHealth integration).

## Data Sharing & Disclosure

- **On-Chain Public Data**: All workout logs, SBTs, and reward claims are permanently recorded on the Solana blockchain and viewable by anyone via explorers (Solscan, etc.).
- **No Third-Party Selling**: We never sell your personal data.
- **Service Providers**: Minimal — only Solana RPC nodes (Helius devnet/mainnet) for transaction submission. No analytics SDKs (Firebase, etc.) are used.
- **Legal Requirements**: We may disclose information if required by law, court order, or to protect our rights.

## Data Security

- All sensitive operations (wallet signing, transaction building) occur inside the secure Seed Vault on the Seeker phone.
- No private keys ever leave the device.
- On-device data (AsyncStorage, Redux) is encrypted at rest where possible.
- On-chain data is immutable by design.

## Your Rights & Choices

- **Delete Data**: You can reset your local progress in Settings. On-chain data (SBTs, workout history) cannot be deleted due to blockchain immutability.
- **Opt-Out of Rewards**: Toggle "Rewards Enabled" off in Settings (calls `toggle_rewards` instruction).
- **Location Permission**: Revoke GPS access anytime in phone settings. Workout tracking will be disabled.
- **Wallet Disconnect**: Use the built-in disconnect button — clears Redux state only.

## Children's Privacy

The App is not intended for children under 18. We do not knowingly collect data from minors.

## Changes to This Policy

We may update this Privacy Policy from time to time. Material changes will be announced via the in-app news feed (on-chain update_daily_news instruction). Continued use after changes constitutes acceptance.

## Contact Us

For privacy questions, data requests, or to report issues with **SanitasSeeker**, **Optima Sanitas** is reachable as follows. We do **not** maintain a separate public product website for this app.

- **Preferred:** [**X** — @optima_sanitas](https://x.com/optima_sanitas)  
- **Email:** [optimasanitas@atomicmail.io](mailto:optimasanitas@atomicmail.io)  

**On-chain program (reference):** `AwZRzJmcbRx3weqFXUi3MWhaEsS6a7GjvkCJH2DUTkhN`

---

*This Privacy Policy ensures full transparency regarding data collection (GPS, exercise metrics, wallet interactions) and protection mechanisms for SanitasSeeker users.*
