# SanitasSeeker + LEV — single documentation set

## Three connected Git repositories (check on every push)

Product work spans **three clones** (different GitHub remotes). After a session, verify **`git status`** and **`git push`** in **each** repo you changed—especially when editing **`programs/*/src/lib.rs`** here.

| Local path (typical) | GitHub | Role |
|----------------------|--------|------|
| `../App/` | **OptimaSanitas/lev-app** | Sanitas Seeker — **exercise** React Native app. |
| `../../Optima_newApps/SeekerMobileCalc` (or your Calc clone) | **OptimaSanitas/SeekerMobileCalc** | Sanitas Calc — calculator + Solana tab (companion). |
| **this repo** (`fitness-sbt/`) | **OptimaSanitas/lev-anchor** | Anchor — **this** program source. |

Calc repo handoff with the same checklist: **`SeekerMobileCalc/CONTINUATION.md`** → section *Three connected Git repositories*.

**Handoff (2026-04-26):** Legal + terms here are the **only** copy for the exercise app and program; **`lev-anchor`** on GitHub mirrors this for stores (`App` → `src/legalUrls.ts`); **push** `main` on **`OptimaSanitas/lev-anchor`** when you change these files. **`App/docs/`** only holds app-specific QA / Play text — no duplicate PRIVACY here. Wider org notes: parent **`../CONTINUATION_LOG.md`**.

**Canonical copy lives here** (`fitness-sbt/docs/`) for both:

- the **SanitasSeeker** exercise app (sibling: `../App/`), and  
- the **Anchor** program in this repository (published to GitHub as **`OptimaSanitas/lev-anchor`** for stores and in-app legal URLs).

**Do not** add duplicate `PRIVACY_POLICY.md`, `TERMS.md`, `LICENSE.md`, or `COPYRIGHT.md` under `App/docs/`. The app’s `../App/src/legalUrls.ts` points at **`lev-anchor`** blob URLs; after editing any legal file here, **push the same content** to the `lev-anchor` remote’s `main` branch (or your published mirror) so listings stay accurate.

| Doc | Role |
|-----|------|
| [PRIVACY_POLICY.md](./PRIVACY_POLICY.md) | Store + in-app privacy |
| [TERMS.md](./TERMS.md) | Terms of use |
| [LICENSE.md](./LICENSE.md) | MIT (repo / program framing) |
| [COPYRIGHT.md](./COPYRIGHT.md) | Ownership statement |
| [privacy.html](./privacy.html) / [terms.html](./terms.html) / [index.html](./index.html) | Optional GitHub Pages (if enabled) |
| [LEGEND_MINT_RESTORE_AND_TEST.md](./LEGEND_MINT_RESTORE_AND_TEST.md), [SKR_STAKING_INTEGRATION.md](./SKR_STAKING_INTEGRATION.md), … | Program & integration notes |

**App-only** checklists and release text stay under **`../App/docs/`** (e.g. manual QA, Play console copy)—see `../App/docs/README.md`.
