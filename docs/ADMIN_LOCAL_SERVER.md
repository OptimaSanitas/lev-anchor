# Local admin server (HTML + wallet)

Wallet extensions (Phantom, Solana Mobile, etc.) often **block** or mishandle `file://` pages. Serve the repo from **http://127.0.0.1** instead.

## Run

From this directory (`fitness-sbt`):

```bash
npm run admin:serve
```

Defaults:

- **URL:** [http://127.0.0.1:8787/](http://127.0.0.1:8787/) (opens `admin-final.html`)
- **Port:** override with `PORT=9000 npm run admin:serve`
- **Bind:** `127.0.0.1` only (local machine). Use SSH tunnel if you need remote access.

## Which HTML

| File | Purpose |
|------|--------|
| `admin-final.html` | **Optima admin (single page):** (1) **3-of-5 governor** — init gate, approvals, timelock, **execute upgrade**; (2) **Sanitas Ops** (`sanitas_ops`) — `initialize` / `set_governance`, **recover buffer rent** (`close_buffer_*`), **extend** `ProgramData` (see `SeekerMobileCalc/onchain/sanitas-ops`). Same wallet + RPC for both sections. |

Governor program id and target app id are set in the page (must match `programs/governor` and `programs/fitness-sbt` `declare_id!`). See `PROGRAM_IDS.md`.

## Multisig upgrade flow (summary)

1. `anchor build` → `solana program write-buffer target/deploy/sanitas_seeker.so --url devnet` → copy **buffer** pubkey.
2. `solana program set-buffer-authority <BUFFER> --new-buffer-authority <UPGRADE_GATE_PDA> --url devnet` if needed (buffer authority must be the gate PDA).
3. Open the local URL → connect a **signer** wallet → **Approve** (3 distinct signers).
4. If timelock is set, wait; then **Execute Upgrade** with buffer + **Target Program ID** = `AwZRz…` (`sanitas_seeker`).

Full checklist: **`MAINNET_UPGRADE_PROCESS.md`** and **`PRE-FLIGHT_CHECKLIST.md`** (devnet-safe sections apply the same mechanics).

## After upgrade: app program inits

Upgrading `sanitas_seeker` does **not** auto-create `initialize_calc_fund` or other new PDAs. Run those instructions with the **app** authority workflow (CLI, tests, or a thin script) once the new binary is live.

## Optional: auto-approvals (devnet only)

See `npm run devnet:approve-gate` in `package.json` and `PROGRAM_IDS.md` — **only** when your runbook allows automated signers on devnet.
