# Future: second reward mint (“until it runs out” + later pool rewards)

**Status:** Not implemented in beta-minimal. This doc is the **extension contract** so a ≤2‑week upgrade can add it without guessing.

## Product intent
- **Token A (today):** `SKR` — existing bootstrap → reward pool claim flow.
- **Token B (later):** optional second SPL (or Token-2022) mint used for a **separate campaign** (e.g. “runs until vault empty”) and/or **post‑bootstrap pool rewards**, without mixing balances with SKR.

## On-chain shape (recommended when you build it)
1. **`MintConfig` extension** (or small `RewardPolicy` PDA): store `secondary_mint: Pubkey`, `secondary_bootstrap_enabled: bool`, `secondary_rewards_enabled: bool`, maybe `secondary_claim_amount: u64`.
2. **New PDAs** (mirror today’s pattern): e.g. `secondary-bootstrap-pool`, `secondary-reward-pool` **or** reuse naming with version suffix — each with **ATA** for token B.
3. **`claim_*` v2** or same instruction with **enum / flag** so signers pass the correct vault accounts; keep **SKR path** unchanged for backward compatibility.
4. **Init ix:** `initialize_secondary_pools` (authority-gated) — **no-op until** mint B is set and funded.

## Why we are **not** adding empty fields in `MintConfig` yet
Changing `MintConfig` byte layout **today** shifts every **raw offset** the app reads and breaks **already-initialized** devnet `mint-config` accounts unless you migrate. For “ability there,” **document first**; add fields in the **same upgrade** that also ships pool PDAs + ix.

## Off-chain
- Indexers / app need to show **two** balances if both campaigns are live.
