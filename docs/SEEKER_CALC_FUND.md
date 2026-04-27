# Seeker Mobile Calc — `seeker-calc-fund` PDA

Separate from exercise **bootstrap** / **reward-pool** balances so ops can fund and review **Seeker Mobile Calc** micro-claims without mixing them with sprint rewards.

## On-chain

| Item | Seed / account |
|------|----------------|
| Fund PDA | `[b"seeker-calc-fund"]` — `CalcFund` (authority + bump) |
| Vault | SKR **ATA** owned by `seeker-calc-fund` (created in `initialize_calc_fund`) |
| Per-user day gating | `[b"user-calc-claim", user]` — `UserCalcClaim` (`last_claim_day`) |
| Instruction | `initialize_calc_fund`, `claim_calc_skr` |

`claim_calc_skr` uses the same **game-day** bucket and **0.05 SKR** amount as `claim_daily_skr`, requires **Seeker genesis** in wallet, respects global **`rewards_enabled`** on `mint-config`, and does **not** increment `minted_phase1` (exercise cohort).

**Funding:** after `initialize_calc_fund`, send SKR to the vault ATA with a normal SPL transfer (or your ops script). No separate “deposit” instruction is required for v1.

**Deploy:** upgrading the program is required before these instructions exist on a cluster. Update the client `BETA_CALC_SKR_CLAIMS_OFF` flag in `SeekerMobileCalc` when you intend the in-app CTA to appear.

## Compliance (product, not legal advice)

A **dedicated PDA** improves **transparency and accounting** (pool is visible on-chain and separable from exercise). It does **not** by itself satisfy app-store, promotion, or securities rules. Keep user copy **accurate** (no guaranteed “free daily” unless true), disclose **rules and limits**, and get **jurisdiction-appropriate** review for giveaways or funded pools. See the [Solana Mobile Publisher Policy](https://docs.solanamobile.com/dapp-store/publisher-policy) (no misleading content; regulated services need compliance).
