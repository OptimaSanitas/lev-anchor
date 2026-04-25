# Legend mint (`mint_sbt`) — restore vs merge (devnet / mainnet)

## Status (merged Path A)

The **current** `programs/fitness-sbt/src/lib.rs` now includes:

- **`mint_sbt`** + **`MintSbt`** — genesis gate (`GenesisGateConfig` + `token_interface` genesis mint/ATA), free cohort while `mint_config.minted_phase1 < 1_000`, paid path 1 SKR split (stake / reward pool / dev ATA), **`legend_supply.minted`** vs `LEGEND_INSTRUMENT_CAP` (10k).
- **`extend_legend`** — genesis-gated **free** 30-day extension on **`LegendEntitlement`** (no SKR). Current **`status_holder`** signs; pass **`visual_owner`** (SBT owner) for the PDA seed (often the same pubkey as holder for self-extend).
- **`record_legend_entitlement`** — call in the **same transaction** right after **`mint_sbt`** (keeps `MintSbt` under BPF stack limits); one-time `init` of the entitlement PDA.
- **`transfer_legend_status`** — **`visual_owner`** signs; sets **`status_holder`**; clears **`is_legend`** on the visual owner’s **`user_exercise`** (image / SBT stay on the visual owner’s wallet).
- **`accept_legend_status`** — new **`status_holder`** signs; syncs legend flags into **their** **`user_exercise`** for the same exercise id.
- **`list_legend_sale`** / **`cancel_legend_sale`** / **`buy_legend_sale`** — optional **trustless** path: **`LegendSale`** PDA seeds **`[b"legend-sale", visual_owner, exercise_id]`**; seller lists SKR price; buyer pays **`visual_owner`** ATA and receives **`status_holder`** on **`LegendEntitlement`** (sale account closed to buyer). **`buy_legend_sale`** clears the seller’s **`user_exercise`** legend fields; the buyer should still call **`accept_legend_status`** if the app relies on **`user_exercise.is_legend`** for them.

The Seeker app wires **`mintNewSBT`** / **`extendLegend`** to the IDL account order in `target/idl/sanitas_seeker.json` (see `App/App.tsx`). Manual instruction builders for list/cancel/buy live in **`App/App.tsx`** (discriminators + Borsh layout); UI wiring is optional.

**On-chain pitfall:** `dev_fee_authority` is a fixed pubkey. If the **signing user** is that same wallet, the user SKR ATA and dev SKR ATA are identical → Anchor **`ConstraintDuplicateMutableAccount`**. Real users mint with a wallet **different** from the dev fee recipient; local tests use a **separate minter keypair** (see `tests/legend_mint_local.e2e.ts`).

---

## Trustless sales vs P2P (both coexist)

- **P2P (unchanged):** `transfer_legend_status` then `accept_legend_status`; payment remains **off-chain**.
- **On-chain listing:** `list_legend_sale` / `cancel_legend_sale`; **`buy_legend_sale`** moves **`status_holder`** and SKR in one buyer-signed instruction. A **separate** escrow program could still **CPI** into the P2P path if you want a different payment model; **`LegendEntitlement`** seeds stay **`[b"legend-entitlement", visual_owner, exercise_id]`** (do not replace **`visual_owner`** in the seed).

`mint_sbt` + `record_legend_entitlement` still compose in **one transaction**; marketplace flows can append SPL ixs in the same tx as needed.

---

## Important: archive is **not** a drop-in on today’s program

`sanitas_seeker_lib_full_2026-04-24.rs` is a **snapshot** of `lib.rs` *before* the beta-minimal strip. It **does not** include what the **current** deployed program and app rely on, for example:

- **`genesis-gate` PDA** + `token_interface` Seeker genesis (current app / `claim_daily_skr` / `log_workout`)
- **`user_state`** + **≥1 set** gating on claims, `claim_day` offset
- **`legend-supply`** PDA + `record_legend_mint` / 10k instrument cap
- **Pool split** (`pool-split`, `split_reward_pool_excess`, stake vault init)
- **`treasury-policy`** PDA
- **`get_available_legend_slots`** on minimal reads **`mint_config.minted_phase1`**; the archive reads **`exercise_config.minted_legends`**

**Replacing** `programs/fitness-sbt/src/lib.rs` with the archive file **as-is** would **regress** those behaviors and **break** the Seeker app against the same program id unless you also revert the app and re-init all PDAs (usually unacceptable).

So: **“restore then test Legend mint”** means either **merge** or **isolated devnet fork**, not a blind copy.

---

## Path A — **Merge** `mint_sbt` into the **current** program (done)

1. ~~Port `mint_sbt`, `MintSbt`, `extend_legend`, `ExtendLegend`~~ **Done.**
2. ~~Genesis via `GenesisGateConfig`~~ **Done.**
3. ~~Legend counters: `mint_config.minted_phase1` (1k cohort) + `legend_supply.minted` (10k cap)~~ **Done** (no `exercise_config` for mint).
4. `serde` / `serde_json` — only if you restore JSON-validated `update_daily_news`; otherwise unchanged.
5. `anchor build` → `anchor upgrade` same `declare_id!`.
6. ~~IDL + app discriminators / account metas~~ **Done** (re-verify after each program change).

---

## Automated local e2e (`anchor test`)

- **`Anchor.toml`**: `[provider] cluster = "localnet"` so tests use `solana-test-validator` with a **fixture** SKR mint at `DCrf…` (`tests/fixtures/dcrf_skr_mint.json`). **Deploy / upgrade to devnet:**  
  `anchor deploy --provider.cluster devnet`  
  `anchor upgrade --provider.cluster devnet`
- **`tests/legend_mint_local.e2e.ts`**: Creates a lab SPL genesis mint, inits PDAs, uses a **separate minter** from the fee wallet, calls **`mint_sbt`** (free cohort), asserts `sbt_account.owner`, `mint_config.minted_phase1`, `legend_supply.minted`, and `get_legend_mint_remaining` view; also **`listLegendSale`** / **`cancelLegendSale`** on the entitlement.
- **`tests/sanitas_seeker.ts`**: Idempotent init smoke tests; `before` hook airdrops SOL on localnet when balance is low.

```bash
cd fitness-sbt && anchor test
```

---

## Path B — **Isolated devnet** “Legend lab” (separate program id)

1. Copy archive `lib.rs` into a **new** Anchor program (new `declare_id!`) or a branch that deploys only to devnet under a **new** program address.
2. Point a **throwaway** client at that id to exercise **`mint_sbt`** only.
3. Do **not** expect the production **Sanitas Seeker** app to work against that binary without major app forks.

Useful only for **bytecode / instruction layout** experiments, not for “real” Seeker + genesis-gate testing on one app.

---

## Path C — **Test today without `mint_sbt`**

- **SKR claim:** already supported on minimal — fund pools, genesis gate, `BETA_SKR_CLAIMS_OFF` off — see `App/docs/MANUAL_TEST_CHECKLIST.md`.
- **Legend counter only:** use **`record_legend_mint(n)`** (admin) + **`get_legend_mint_remaining`** to validate the **10k cap** path.

---

## Devnet / production: manual Legend mint checklist

1. **Genesis gate** PDA holds the Seeker genesis **mint** pubkey; user holds ≥1 genesis token in the matching Token-2022 ATA.
2. **`initialize_legend_supply`**, **`initialize_stake_vault`**, reward pool + bootstrap + stake-vault **SKR ATAs** exist and are funded as needed.
3. **`log_workout`** if other flows still require `user_state.sets_completed ≥ 1`.
4. **Free cohort:** `mint_config.minted_phase1 < 1_000` — no SKR spend.
5. **Paid cohort:** user SKR ATA ≥ **1 SKR**; stake-vault SKR ATA must exist (`init_if_needed` in `mint_sbt` can create it, but stake-vault **PDA** must exist via `initialize_stake_vault`).
6. **App:** mint tx uses `fetchSeekerGenesisMint` + correct account order; after upgrade, confirm Solscan instruction accounts match `target/idl/sanitas_seeker.json`.

### Devnet trace template (fill after a successful mint)

| Field | Value |
|--------|--------|
| Cluster | `devnet` |
| `mint_sbt` signature | _(paste)_ |
| Explorer | `https://solscan.io/tx/<sig>?cluster=devnet` |
| SBT PDA | _(user + `user-sbt` + exercise id)_ |
| `legend_supply.minted` after | _(optional: `get_legend_mint_remaining` / custom read)_ |

### `extend_legend` on devnet

Requires ≥ **1 SKR** in the user’s SKR ATA (same mint as program), genesis gate + user genesis ATA, `user_exercise` PDA for the exercise id, reward pool + pool SKR ATA. Not covered by the local e2e (no mint authority for cloned SKR on local fixture).

---

## References

- Archive copy + old restore blurb: `fitness-sbt/archive/README.md`
- Beta-minimal scope: `fitness-sbt/BETA_MINIMAL.md`
- Devnet claim vs staking: `App/docs/MANUAL_TEST_CHECKLIST.md` § *Devnet: SKR claim vs Legend mint vs Solana Mobile staking*
