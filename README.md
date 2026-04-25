# LEV Anchor Program

**Live Exercise Verification (LEV)** — Soulbound Fitness Token program on Solana.

**Repo:** this directory is the **public** Anchor workspace (remote is commonly `OptimaSanitas/lev-anchor`). The **React Native client** is the sibling **`../App/`** tree (**private** `lev-app`). Operator-only files `MAINNET_UPGRADE_PROCESS.md`, `PRE-FLIGHT_CHECKLIST.md`, and `scripts/preflight-upgrade.sh` are **gitignored** here; keep local copies for your own upgrades.

## Overview

This Anchor program powers the LEV mobile app. It issues non-transferable Soulbound Tokens (SBTs) to users who complete verified walk-run exercise sessions using GPS speed detection on the Solana Seeker phone.

### Core Features

- Phased SBT minting (1000 tokens per version)
- First 1000 users receive "Early Adopter" SBT (`is_early = true`)
- Public fitness tracking (total distance walked, distance ran, sets completed)
- Prepared for future encrypted fitness data via MagicBlock

## Program ID

`BUPY7yPt6BqWUTHmqLteEfRbH9zH8zQMcUNA9NRBFYEz`

## Build & Deploy

```bash
anchor build
anchor test
anchor deploy --provider.cluster devnet
Available Instructions

mint_sbt(version, uri) — Mint a new SBT (limited to 1000 per version)
update_fitness_stats(walked, ran, sets) — Update public fitness metrics after each set
update_encrypted_fitness(encrypted_data) — Future instruction for MagicBlock encrypted data
update_sbt_uri(new_uri) — Update public metadata URI (image, description, etc.)

Repository Structure
textlev-anchor/
├── programs/
│   └── fitness_app/
│       └── src/
│           └── lib.rs          # Main program logic
├── Anchor.toml
├── Cargo.toml
└── README.md
Security Notes

All sensitive updates require owner signature
PDAs used for user-specific accounts
Phased minting prevents unlimited issuance
Designed for future zero-knowledge / encrypted extensions


Ready for review by the Solana Seeker team.
License: MIT
