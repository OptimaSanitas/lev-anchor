# SKR staking vs `sanitas_seeker` (Anchor) — integration notes

This doc is about **Solana Mobile’s SKR** (Seeker ecosystem SPL staking), **not** native **SOL** stake accounts (`solana_program::stake`). SKR uses a **separate on-chain program** for stake / unstake / rewards timing.

## Sanitas scope — treasury / company SKR only

**Sanitas is not staking SKR on users’ behalf.** Any Solana Mobile SKR staking you pursue is for **company-controlled SKR**, for example:

- **Treasury** balances (multisig, ops wallet, cold custody you define off-repo).
- **Accumulated SKR** from **Legend** (and related) **mints and sales**.
- **Future** revenue streams you route into treasury in SKR (e.g. **online store**, or **X / social** viewership or partner payouts **if** you implement them successfully).

End-user **daily claim** SKR (`claim_daily_skr`) is a **transfer out of your pools to the user**; that is unrelated to whether treasury later stakes **its own** SKR through Solana Mobile’s program. Keep **custody, disclosures, and securities/marketing** review with counsel—this repo only describes technical integration options.

## Treasury policy — canonical: **multisig + timelock** (your ops layer)

Because the staked SKR is **yours** (treasury / company), **how** you use staking **rewards** is under **your governance**—not something Solana Mobile’s app defines for you.

**Sanitas aligns with the common industry pattern: multisig + timelock** (not permanent lock). Concretely:

1. **Primary control — real money:** Use a **3-of-5 multisig** (Squads, Safe, or native) as the only signer that can **stake, unstake, or transfer** treasury SKR. Put a **timelock / delay** in front of those transactions (typical public examples range **~48h–7d** depending on risk tier) so stakeholders see queued actions before execution.
2. **Claim or accrue** rewards per Solana Mobile’s staking rules; then **allocate**: part **distribute** (e.g. pools, grants), part **compound** (restake).
3. Same five people as **`upgrade_governor`** (`MAINNET_UPGRADE_PROCESS.md`) or a **separate** treasury multisig — document which wallet is canonical for treasury SKR.

**On-chain mirror (in `sanitas_seeker`):** PDA **`treasury-policy`** is a **second** timelock layer for **policy parameters** only: **`distribute_bps`**, **`authority`** (= multisig vault pubkey), **`min_change_interval_sec`** between edits to that intent. Instructions: **`initialize_treasury_policy`** (mint-config authority once) → **`update_treasury_policy`**, **`update_treasury_policy_timelock`**, **`set_treasury_policy_authority`** (signer **`authority`**). Does **not** move SKR or CPI to Solana Mobile staking. See `BETA_MINIMAL.md` and `App/admin-skr-mint.html`.

Exact **percentages**, **cadence**, and **which pool gets topped up** when you actually move SKR remain a board/treasury decision; the staking program + SPL transfers are what settle value.

## Canonical on-chain IDs (mainnet — Solana Mobile)

From Solana Mobile’s public post **“SKR is Live”** ([blog](https://blog.solanamobile.com/post/skr-is-live)):

| Item | Address |
|------|---------|
| **SKR mint** (SPL) | `SKRbvo6Gf7GondiT3BbTfuRDPqLWei4j2Qy2NPGZhW3` |
| **SKR staking program** | `SKRskrmtL83pcL4YqLWt6iPefDqwXQWHSw9S9vz94BZ` |

Product UX: [https://stake.solanamobile.com/](https://stake.solanamobile.com/) · Overview: [https://solanamobile.com/skr](https://solanamobile.com/skr)

**Devnet:** this repo’s app/admin often use a **different SKR mint** for testing (e.g. `DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ` in `App/admin-skr-mint.html`). Staking program layout may only exist on **mainnet-beta**; verify before shipping.

## What “Anchor handling staking” can mean

### 1) Client-only (TypeScript / React Native) — most common for treasury

**Treasury or admin tooling** (or a dedicated internal script) builds transactions that call **`SKRskrmt…`** with a **company-controlled** signer (multisig, treasury hardware wallet, etc.). The consumer **Seeker app** path does not need to expose staking for pool SKR unless you explicitly choose to. Your **`sanitas_seeker`** program is unchanged.

- Solana Mobile’s **Anchor + Mobile Wallet Adapter** recipe: [Anchor integration guide](https://docs.solanamobile.com/recipes/solana-development/anchor-integration) (wallet + `Program` + `.rpc()` / manual `Transaction`).
- If you obtain an **IDL** for `SKRskrmt…`, you can `new Program(idl, SKR_STAKING_PROGRAM_ID, provider)` and call `.methods....` like any other Anchor program.
- **IDL discovery:** standard `anchor idl fetch SKRskrmtL83pcL4YqLWt6iPefDqwXQWHSw9S9vz94BZ --provider.cluster mainnet` currently fails (no Anchor IDL account at the default on-chain IDL PDA in this environment). Next steps:
  - Ask **Solana Mobile** (developer relations / dApp Store channel) for a published IDL or SDK.
  - Inspect **real staking transactions** from `stake.solanamobile.com` in an explorer (instruction discriminators + account order), then either hand-build `TransactionInstruction`s or reconstruct a minimal IDL.
  - Search for community SDKs (verify authenticity against on-chain program).

### 2) CPI from your own Anchor program — possible but heavier

Here your program **`invoke`s** the SKR staking program. You need:

- Exact **instruction data** and **account list** for stake / unstake / claim (from IDL or reverse engineering).
- A **signer model** the staking program accepts. Many SPL-stake designs require the **token authority** (owner of the SKR source account) to sign. A **PDA** owned by `sanitas_seeker` can sign via **PDA seeds** in your program **only if** the staking program was written to accept your program as a signer / delegate. Do **not** assume that without verifying accounts on-chain.
- **Reward accounting:** rewards may be represented as extra SKR minted to a vault, or as a separate “rewards” token account, or updated position state — depends entirely on `SKRskrmt…` layout.

If PDAs cannot participate, the practical pattern is: **treasury multisig or hot wallet** stakes via (1); your program only manages **plain SPL** pools you already have.

### 3) Operational (no new CPI)

Treasury moves **company SKR** (from Legend/store/social pipelines, or withdrawn from pool PDAs by authority) into a **controlled wallet**, stakes via official UI or a script using (1), and **periodically** sends SKR back to fund `bootstrap-pool` / `reward-pool` ATAs if that matches your treasury policy. Your Anchor code stays as today; finance/ops owns the staking flow.

## SOL staking vs SKR staking (anchor of mental model)

| | **SOL** | **SKR** |
|---|--------|--------|
| Program | Native `Stake1111…` stake program | `SKRskrmtL83pcL4YqLWt6iPefDqwXQWHSw9S9vz94BZ` |
| Asset | Wrapped native SOL / stake accounts | SPL mint `SKRbvo6Gf7GondiT3BbTfuRDPqLWei4j2Qy2NPGZhW3` |
| Docs | Solana validator / stake docs | Solana Mobile blog + `solanamobile.com/skr` |

Do not reuse SOL-stake tutorials for SKR; the program and accounts differ.

## Suggested next engineering steps

1. **Confirm cluster** (mainnet vs devnet) and **mint** you integrate in production vs test.
2. **Obtain interface** to `SKRskrmt…` (IDL, SDK, or decoded instructions from production txs).
3. Decide **signer**: user wallet only, multisig, or PDA-capable CPI — based on what the staking program actually requires.
4. If CPI from `sanitas_seeker`: add a **thin module** of `invoke_signed` / Anchor `Program` CPI with **fuzzed + mainnet-fork tests** once you have the IDL.

## Related in this repo

- Third-party SKR disclaimer + ops note: `BETA_MINIMAL.md` section **SKR (third party — Solana Mobile)**.
- Pool PDAs do **not** call SKR staking today: `programs/fitness-sbt/src/lib.rs` pool-economics comments.
- **`treasury-policy` PDA** (`initialize_treasury_policy`, `update_treasury_policy`, …): `target/idl/sanitas_seeker.json` and admin panel **Treasury policy** section.
