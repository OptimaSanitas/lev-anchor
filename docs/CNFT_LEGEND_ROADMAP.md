# Roadmap: frame cNFT vs legend instrument (engineering invariant)

**Hard cap (locked):** **≤ 10,000 legend instruments ever** (global, all time). **Implemented (beta-minimal):** PDA **`legend-supply`** (`LegendSupply.minted`), `LEGEND_INSTRUMENT_CAP = 10_000`, admin **`record_legend_mint`**. Future **legend mint** instruction must increment this counter (only path) and reject at cap. No `MintConfig` resize. No “overflow” tree capacity needed beyond what holds **10k legend leaves** (plus any buffer you want for burns/reorgs — usually none).

**Invariant (do not blur in product or on-chain):**

- **Frame cNFT** — image / video-frame credential per `(exercise_id, video_id, frame_index)` (compressed NFT in a Merkle tree). Economics: free tier windows, optional **1–10 SKR** (or similar) for extra frames *after* promos; “paying for pixels / proof.” **Not** capped at 10k unless you add a separate product rule; capacity is its own tree sizing problem.
- **Legend instrument** — **separate** tradable asset (second cNFT collection or SPL). Economics: status, transfers, secondary; “paying for status.” Must **not** reuse the frame mint price as the legend price. **Capped at 10,000 total mints ever.**

**Product sketch (target, not yet in beta-minimal binary):**

- User starts with **one mint per `exercise_id`** for separate tracking.
- **First published video** `V0` per exercise: frames `0..999` (example) tied to **legend eligibility rules**; later videos get new `video_id` and their own promo counters.
- **Legend status can move** by trading the **legend instrument**, not by selling the frame image users “keep.”
- **Legend tree sizing:** `2^13 = 8,192` **< 10,000** → use at least **`maxDepth = 14`** (`2^14 = 16,384` leaves) for the **legend-only** Merkle tree, unless you use a deeper tree for headroom. Metaplex calculator + mobile tx-size checks for canopy/buffer.
- **Frame trees** (pixels): separate sizing from legend; **10 exercises × many frames** may be **one shared frame tree** or multiple trees—do not conflate with the **10k legend cap**.

**Beta-minimal today:** `log_workout`, pools, news, genesis gate — **no** Bubblegum/CPI in this cut. Implement cNFT + legend split in a **later** program upgrade + app release; keep this doc as the single source for direction.

**App note:** configure **`initialize_genesis_gate`** on each cluster so `GenesisGateConfig` points at a real Token-2022 mint (mainnet `GT22…`, devnet = your test mint). The app loads it from the **`genesis-gate`** PDA (`App/src/genesisGate.ts`).
