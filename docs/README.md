# SanitasSeeker + LEV — single documentation set

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
