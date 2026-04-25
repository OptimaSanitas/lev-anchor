# `sanitas_seeker` program IDs (devnet)

## Devnet deploy wallet (Anchor + `solana` CLI)

| | |
| --- | --- |
| **Keypair file** | `~/Groking/projects/sanitas_payer.json` (set in `Anchor.toml` → `[provider]` `wallet`) |
| **Public key** | `B9Qo6q398kvryKQuCUMjRxQHMbVTGTc3wwSbrRoKaTrc` |

Use this keypair for `anchor deploy`, `anchor program deploy`, and funding deploy transactions on devnet. If your projects folder is spelled differently (e.g. `GRoking`), point `wallet` at the same `sanitas_payer.json` on your machine.

**Note:** `Anchor.toml` uses `[provider] cluster = "localnet"` so `anchor test` runs against `solana-test-validator` with a devnet SKR mint fixture. Deploy or upgrade to devnet with an explicit cluster override, for example: `anchor deploy --provider.cluster devnet` and `anchor upgrade --provider.cluster devnet`.

After the **first** successful deploy of `AwZRz…`, `solana program show` should list **Authority** = this pubkey (unless you set another upgrade authority when deploying).

---

## Current (in source — use this App + admin UIs)

| | |
| --- | --- |
| **Program id** | `AwZRzJmcbRx3weqFXUi3MWhaEsS6a7GjvkCJH2DUTkhN` |
| **Declared in** | `programs/fitness-sbt/src/lib.rs` (`declare_id!`) |
| **Anchor** | `Anchor.toml` → `[programs.devnet] sanitas_seeker` |
| **Program keypair** | `target/deploy/sanitas_seeker-keypair.json` (local build artifact; `target/` is gitignored) |

After a **first successful** `anchor program deploy` / `anchor deploy` to devnet with your deploy wallet, that wallet’s pubkey becomes the **on-chain program upgrade authority** (unless you pass a different authority). You can confirm with:

```bash
solana program show AwZRzJmcbRx3weqFXUi3MWhaEsS6a7GjvkCJH2DUTkhN --url devnet
```

Keep the `sanitas_seeker-keypair.json` that matches this program id in a **password manager or offline backup**. If you lose it, you cannot prove program identity for some flows; the upgrade authority is separate (the deployer key) but the program id is derived from the program keypair.

---

## Legacy (retired for this repo)

| | |
| --- | --- |
| **Program id** | `3kvgciqi3Tk9KjZPNk5b6rurCZGELP8oJzViFrGmHPRu` |
| **On-chain upgrade authority (devnet, as reported)** | `AJ2gNkvsivS8rAjBfYNiBLk9KGnUmohNgZBLbmWaoJJf` |
| **ProgramData address (reference)** | `GbBGFPZ1cJXAEjrjd8URm8QTCmHMjUE67nYL6zjGYQof` |

We moved to **`AwZRz…`** so the team can deploy and upgrade using a **new** program keypair under your control, without the lost or mismatched upgrade authority on `3kvgc…`.

**On-chain state** (PDAs, user accounts) from the old program id does **not** carry over. You must re-run initializers (mint config, reward pools, daily news, etc.) against `AwZRz…`.

A copy of the **old** program keypair (for `3kvgc…`) can be kept locally as:

`target/deploy/sanitas_seeker-keypair.legacy-3kvgciqi3.json` (if you created that backup) — only on your machine; not committed.

---

## Governor (separate binary)

`upgrade_governor` has its own `declare_id!` in `programs/governor` — do not confuse with `sanitas_seeker`. See `PRE-FLIGHT_CHECKLIST.md` for gated upgrades.

**Devnet:** to auto-submit **3 of 5** gate approvals using `~/Groking/projects/sanitas_signer{1,2,3}.json` (indices 0, 1, 3), see `MAINNET_UPGRADE_PROCESS.md` §4.3 and run `npm run devnet:approve-gate` in `fitness-sbt` with `SANITAS_DEVNET_ONLY=1`.

---

## Program upgrade: “account data too small” / ProgramData length

If `anchor deploy` / `anchor upgrade` fails with **ProgramData account not large enough**, extend the buffer **before** retrying (upgrade authority pays rent for the extra bytes):

```bash
solana program show <PROGRAM_ID> --url devnet   # note Data Length vs local sanitas_seeker.so size
solana program extend <PROGRAM_ID> <ADDITIONAL_BYTES> -k ~/Groking/projects/sanitas_payer.json --url devnet
anchor upgrade target/deploy/sanitas_seeker.so --program-id AwZRzJmcbRx3weqFXUi3MWhaEsS6a7GjvkCJH2DUTkhN --url devnet
```

---

## Recovering devnet SOL (buffers + old keypairs)

After failed deploys/upgrades, lamports often sit in **program buffer** accounts owned by your **upgrade authority** (usually `sanitas_payer.json`). Recover them before they feel “lost”:

```bash
# List buffer accounts you can close (upgrade authority = payer keypair)
solana program show --buffers --url devnet

# Close all eligible buffers and return rent to the signer (common after a stuck upgrade)
solana program close --buffers --keypair ~/Groking/projects/sanitas_payer.json --url devnet
```

Also sweep any **one-off test keypairs** (old deploy keys, faucet wallets, `sanitas_signer*.json` if you only used them for devnet gate tests):

```bash
solana balance -k <KEYPAIR>.json --url devnet
# Sweep a secondary wallet to the main devnet payer (signer = source wallet):
solana transfer B9Qo6q398kvryKQuCUMjRxQHMbVTGTc3wwSbrRoKaTrc ALL \
  --from ~/Groking/projects/sanitas_signer1.json \
  -k ~/Groking/projects/sanitas_signer1.json \
  --url devnet
```
(Replace paths with whatever keypair holds the stray SOL; `ALL` leaves enough for the fee.)

**Program id keypair** (`sanitas_seeker-keypair.json`): the **program id address** is not a normal wallet you “empty”; rent lives in the **program data** account. You recover SOL from **buffers** and from **wallets you control**, not from the program’s own executable account (that only returns rent if the program is **closed** by authority, which you normally do not do on a live id).

**Legacy program `3kvgc…`:** if you still hold the old upgrade authority key and there is reclaimable state on devnet, treat that as a separate audit (`solana account …`, buffers under that authority). On-chain PDAs for the old id are not portable to `AwZRz…`.
