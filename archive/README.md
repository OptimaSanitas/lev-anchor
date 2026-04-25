# Archived full Sanitas Seeker on-chain program

- **`sanitas_seeker_lib_full_2026-04-24.rs`**: exact copy of `programs/fitness-sbt/src/lib.rs` *before* the **beta-minimal** strip (SBT stack, full JSON news validation, SOL micro-reward, etc.).

## When to restore (≈1–2 weeks or when ready)

**Warning (2026):** this snapshot predates **genesis-gate**, **pool split**, **treasury-policy**, **legend-supply**, and the current **`claim_daily_skr`** / **`get_available_legend_slots`** layout. **Do not** replace today’s `programs/fitness-sbt/src/lib.rs` wholesale—use **`docs/LEGEND_MINT_RESTORE_AND_TEST.md`** (merge `mint_sbt` into the current program, or a separate devnet program id).

1. Replace `../programs/fitness-sbt/src/lib.rs` with this file **only** if you intentionally revert the whole stack (or merge as needed—preferred).
2. Restore `programs/fitness-sbt/Cargo.toml` `serde` + `serde_json` deps if the full program still uses them.
3. From `fitness-sbt/`: `anchor build` → `anchor deploy` or `anchor upgrade` with the **same** program id if you are upgrading the live binary; **on-chain account layouts** (PDAs) must be compatible with any accounts you created in beta, or use a new program + migration.
4. Regenerate the client/IDL in the app (`target/idl/sanitas_seeker.json`).

**Solana program rent** grows with **`.so` size**; a smaller binary costs less to deploy. The minimal build was intended to fit a **~$100–150** (SOL-price-dependent) devnet main deploy budget. Restoring the full `lib` will produce a **larger** binary — plan for higher rent for initial deploy/upgrade or buffer lamports in the program authority wallet.

## Beta minimal behavior (see `BETA_MINIMAL.md` in repo `fitness-sbt/`)

- Daily **SKE-Claim**-style claim from pool (same PDAs/amount as before, plus `rewardsEnabled` gating in `claim_daily_skr`).
- **1 claim per “game day”** per user, using a **UTC+8h offset** by default to approximate a US–Pacific “calendar day” for Seeker users (tweak the constant in `lib.rs` if you change strategy).
- **First-1000** tracking: `mint_config.minted_phase1` increases on a wallet’s **first** successful `claim` (capped at 1000) while the user’s `user_exercise` is still brand-new; others can still claim while pools have balance.
