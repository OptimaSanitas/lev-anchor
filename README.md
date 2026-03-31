# LEV Anchor Program

**Live Exercise Verification (LEV)** — Soulbound Fitness Token program on Solana.

## Overview
- Phased SBT minting (1000 tokens per version)
- First 1000 users receive "Early Adopter" SBT
- Tracks fitness activity (distance walked, distance ran, sets completed)
- Designed for future encrypted data storage via MagicBlock

## Program ID
`BUPY7yPt6BqWUTHmqLteEfRbH9zH8zQMcUNA9NRBFYEz`

## Build & Deploy
```bash
anchor build
anchor test
anchor deploy --provider.cluster devnet
