# Beta-minimal Sanitas Seeker on-chain program

## What changed

- **Full source snapshot**: `archive/sanitas_seeker_lib_full_2026-04-24.rs` (and `archive/README.md`). **Nothing was deleted** from history—this is a copy; restore by replacing `programs/fitness-sbt/src/lib.rs` and restoring `serde` + `serde_json` in `programs/fitness-sbt/Cargo.toml` if the full file still needs them.
- **Stripped** for a smaller deploy binary and lower program-data rent: all **SBT / Legend** flows, **SOL** `claim_daily_reward`, `initialize` counter, and **JSON validation** in `update_daily_news` (news is still raw bytes, max 34k; app remains compatible).
- **Kept** (same program id, same major PDAs): `mint-config`, `reward-pool` / `bootstrap-pool`, `user-state` (log), `user-exercise` (claim), `daily-news-seeker-final`, `toggle_rewards`, `admin_mint_skr`, `get_available_legend_slots`.
- **Legend instrument cap (10k):** PDA seeds **`legend-supply`** — `initialize_legend_supply` (mint-config authority), `record_legend_mint(n)` (admin, for future Bubblegum / `mint_sbt`), `get_legend_mint_remaining`. Separate from **claim cohort** `minted_phase1` (1k UI). See `docs/CNFT_LEGEND_ROADMAP.md`.

## SKR (third party — Solana Mobile)

- **SKR** is **Solana Mobile’s** Seeker ecosystem token (reward / governance surface for the phone ecosystem). **This project does not issue or control SKR**; the program and app **integrate** it for workouts, pools, and claims.
- **Staking and inflationary rewards** for SKR are defined and operated by **Solana Mobile** (e.g. their public staking product at [https://stake.solanamobile.com/](https://stake.solanamobile.com/) and their blog/docs). Nothing in `sanitas_seeker` CPIs into that staking stack; PDAs here hold plain SPL balances.
- **Engineering note (Anchor + SKR staking program):** see `docs/SKR_STAKING_INTEGRATION.md` (mainnet program id + mint, client vs CPI, IDL discovery, PDA signer caveats).
- **Product / treasury yield (not user delegation):** any SKR staking you do is for **company / treasury SKR**—e.g. treasury reserves, SKR accumulated from **Legend mints and sales**, and **future** SKR from an **online store** or **social / X** viewership revenue **if** you route it that way. You are **not** staking on behalf of end users; users who **claim** SKR receive transfers from your pools per `claim_daily_skr`. To earn Solana Mobile staking rewards on **treasury** SKR, move it to **custody you control** and use their staking product—subject to their terms and eligibility. Legal/compliance stays outside this repo.
- **Treasury split / governance (canonical: multisig + timelock):** treasury SKR moves (stake / unstake / transfers) through **3-of-5** + an external **timelock** (e.g. Squads/Safe delay); on-chain **`treasury-policy`** adds a timelock on **distribute vs compound intent** only—see `docs/SKR_STAKING_INTEGRATION.md` § *Treasury policy* (multisig may match `MAINNET_UPGRADE_PROCESS.md` signers or be a separate vault).

## Security (Seeker Genesis + gating)

- **Genesis (Seeker) gate**: `user_genesis_ata` and `genesis_mint` use `anchor_spl::token_interface` via `InterfaceAccount`, so the user must pass a real mint + token account whose mint pubkey matches **`GenesisGateConfig.seeker_genesis_mint`** (PDA seeds **`genesis-gate`**), owned by **legacy SPL or Token-2022** (Seeker SGT is Token-2022). Fake accounts or wrong program owners are rejected. Balance must be ≥ 1; token account owner must be the signer.
- **Deploy / upgrade (required once per cluster):** after deploying or upgrading the program, the **authority** must send **`initialize_genesis_gate(seeker_genesis_mint)`** with the canonical mint for that cluster — **mainnet:** Seeker genesis **`GT22s89nU4iWFkNXj1Bw6uYhJJWDRPpShHt4Bk8f99Te`**; **devnet:** any **Token-2022** mint you control (not the same mint as mainnet). Use **`set_seeker_genesis_mint`** to rotate later (same authority). The app reads the mint from this PDA via `App/src/genesisGate.ts`.
- **CLI helper (devnet):** `SANITAS_DEVNET_ONLY=1 SEEKER_GENESIS_MINT=<base58> yarn devnet:set-genesis-mint` — or first init: add **`INIT_GENESIS_GATE=1`**. See `scripts/set-seeker-genesis-mint.cjs`.
- **At least one set**: `claim_daily_skr` also requires an existing `user_state` PDA with **`sets_completed >= 1`** (from a prior `log_workout` that logged real work), so a raw claim with no exercise history fails with `NeedOneSetForClaim`.

## Behavior

- **`claim_daily_skr`**
  - Requires `mint_config.rewards_enabled`.
  - **One successful claim per “game day”** per user: `user_exercise.last_active_day` vs a day index from `claim_day` (Unix time + `CLAIM_DAY_OFFSET_SEC` in `lib.rs`, default `8 * 3600` to bias the day boundary toward US–Pacific; set to `0` for UTC).
  - **0.05 SKR** from bootstrap pool first, else reward pool; fails with `InsufficientRewardPool` when both are short.
  - **First-1000 tracking**: on a wallet’s **first** claim (new `user_exercise`), if `minted_phase1 < 1000`, increment `minted_phase1`. This does **not** block user 1001+ from claiming if the pool has tokens; it only tracks the cohort counter for UI.
- **`get_available_legend_slots`**: returns `1000 - minted_phase1` (saturating).
- **`initialize_legend_supply` / `record_legend_mint` / `get_legend_mint_remaining`**: PDA **`legend-supply`**, global cap **10_000** legend instruments (admin counter until mint path exists).

## App changes (this repo)

- **Where we’re headed:** separate **frame cNFT** economics from **legend instrument** economics (Metaplex Bubblegum + optional second asset). See `docs/CNFT_LEGEND_ROADMAP.md` — not implemented in this minimal on-chain cut.
- **Mainnet beta:** defer initializing the large **`daily-news`** PDA (~0.24 SOL rent) until needed; use **one workout + one Arweave-backed video** for the first drop. **Second reward mint** extension spec: `docs/FUTURE_SECOND_REWARD_MINT.md` (no layout change yet).
- `claim` instruction account order updated to match `target/idl/sanitas_seeker.json` (adds `mint_config`, **read-only `user_state`** for the ≥1 set check, `system_program`, and `user-exercise` PDA; see IDL).
- Legend slots + mint config reads use `minted_phase1` at offset **41**.
- Claim button no longer uses a `1800` cap; on-chain enforces pool + day + rewards.

## SBT / mint in the app

The **beta-minimal program no longer includes `mint_sbt`**. In-app SBT / Legend mint will fail until **`mint_sbt` is merged back** (or the program is otherwise extended)—the archive file is **not** a safe drop-in on the current binary; see **`docs/LEGEND_MINT_RESTORE_AND_TEST.md`**. Workout + SKR-Claim + news are the supported paths for this cut.

## Reward pool split (1M+ SKR excess)

When the **reward pool token account** balance is **above** `PoolSplitConfig.threshold_raw` (default **1M SKR** raw at 9 decimals = `1_000_000_000_000_000`), anyone may call **`split_reward_pool_excess`**. The program transfers **excess** only (balance minus threshold): **50%** → **stake vault** SKR ATA, **25%** → **bootstrap pool** SKR ATA, **25%** → **treasury** SKR ATA (owner must match `pool_split.treasury`). The reward pool is left with **at least** the threshold.

**Setup (mint-config authority):** `initialize_stake_vault` (PDA **`stake-vault`**) → create/fund **stake vault** SKR ATA if missing → **`initialize_pool_split_config(threshold_raw, treasury)`** (PDA **`pool-split`**; `treasury` is the **owner** pubkey of the treasury SKR ATA). Optional: **`update_pool_split_threshold`**, **`update_pool_split_treasury`**.

**Errors:** `BelowSplitThreshold`, `TreasuryMismatch`, `SplitMathOverflow`.

## Treasury policy PDA (`treasury-policy`)

**Canonical custody model:** **multisig + timelock** for real SKR flows (off this program); this PDA adds **multisig + timelock on policy edits** for transparency. It records **`distribute_bps`** / 10_000 vs **compound intent** (remainder), **`min_change_interval_sec`** between **parameter** updates, and **`authority`** (your **3-of-5 multisig** vault pubkey). **Does not** move SKR or CPI to Solana Mobile staking.

- **`initialize_treasury_policy(authority, distribute_bps, min_change_interval_sec)`** — once; **mint-config authority** only.
- **`update_treasury_policy(distribute_bps)`** — signer must be **`authority`**; respects timelock after first update.
- **`update_treasury_policy_timelock(min_change_interval_sec)`** — same signer + timelock rules.
- **`set_treasury_policy_authority(new_authority)`** — current **`authority`** only; timelock applies.

**Errors:** `TreasuryPolicyTimelockActive`, `InvalidTreasuryPolicy`. **`App/admin-skr-mint.html`** includes buttons (refresh discriminators from `target/idl/sanitas_seeker.json` after upgrades).

## Deploy size / cost (indicative)

- Current minimal `.so` size: **~309 KB** (see `ls -lh target/deploy/sanitas_seeker.so` after `anchor build`).
- Solana program **rent-exempt** balance scales with program size; **fiat $** depends on **SOL/USD** and **network** (devnet vs mainnet). A **~$120** **budget** is a planning number—**measure** the actual rent + fees on your cluster before deploying.

## Upgrade or restore the full program

1. Restore `lib.rs` from the archive (and `Cargo.toml` deps if needed).
2. `cd fitness-sbt && anchor build`
3. `anchor upgrade` with the same **upgrade authority** as the live program, **or** deploy a new program id and migrate the app (only if you intentionally change the id).
4. Refresh the app IDL / discriminators from `target/idl/sanitas_seeker.json` if any instruction or account set changed.
