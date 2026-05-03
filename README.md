# LEV Anchor Program

**Live Exercise Verification (LEV)** — Soulbound Fitness Token program on Solana.

**Repo:** public Anchor workspace for LEV on Solana ([OptimaSanitas/lev-anchor](https://github.com/OptimaSanitas/lev-anchor)). The **Sanitas Seeker** mobile app is a separate client codebase that talks to this program.

### Legal docs (app links)

**Single set of docs** for app + program: all live under **[docs/](./docs/)** in this tree. Do not duplicate them under `../App/docs/`.

The **SanitasSeeker** app opens **Terms** and **Privacy** from the public **`lev-anchor`** copy on GitHub (no Pages required):

- [docs/TERMS.md](docs/TERMS.md) → `https://github.com/OptimaSanitas/lev-anchor/blob/main/docs/TERMS.md`
- [docs/PRIVACY_POLICY.md](docs/PRIVACY_POLICY.md) → `https://github.com/OptimaSanitas/lev-anchor/blob/main/docs/PRIVACY_POLICY.md`

**Optional — GitHub Pages:** If you use **Settings → Pages → Deploy from a branch**, pick branch **`main`** and folder **`/docs`** (not `/ (root)`), or the site will not serve `terms.html` / `privacy.html`. The app does **not** depend on Pages when using the blob URLs above.

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
