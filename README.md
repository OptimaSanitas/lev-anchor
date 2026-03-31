# LEV Anchor Program

**Live Exercise Verification (LEV)** — Soulbound Fitness Token (SBT) program on Solana.

## Overview

This Anchor program powers the LEV mobile app on Solana Seeker. It issues Soulbound Tokens (SBTs) to users who complete verified walk-run exercise sets using GPS-based speed detection.

### Key Features

- **Phased SBT Minting**: 1000 tokens per version (first 1000 users receive "Early" SBT)
- **Fitness Tracking**: Records total distance walked, distance ran, and completed sets
- **Soulbound Design**: Tokens are non-transferable (tied to user's wallet)
- **Future-Ready**: Includes placeholder for encrypted fitness data (MagicBlock/Arcium)

## Program ID

`BUPY7yPt6BqWUTHmqLteEfRbH9zH8zQMcUNA9NRBFYEz`

## Build & Test

```bash
anchor build
anchor test
anchor deploy --provider.cluster devnet
Instructions
mint_sbt
Mints a new SBT to a user. Limited to 1000 per version.
update_fitness_stats
Updates public fitness metrics (distance walked, distance ran, sets completed) after each completed exercise set.
update_encrypted_fitness
Future instruction for storing encrypted fitness data via MagicBlock.
update_sbt_uri
Allows the owner to update the public metadata URI (image, description, etc.).
Repository Structure
textlev-anchor/
├── programs/
│   └── fitness_app/
│       └── src/
│           └── lib.rs          # Main program logic
├── Anchor.toml
├── Cargo.toml
└── README.md
Security & Audit Notes

All accounts use PDAs for user-specific data.
Ownership checks are enforced on all update operations.
Phased minting prevents unlimited issuance.
Designed with future encrypted data storage in mind.

License
MIT

Ready for review by the Solana Seeker team.
