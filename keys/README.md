# Program keypair (local only)

**Devnet fee-payer / deploy wallet** (not the program id keypair): `~/Groking/projects/sanitas_payer.json` — see `../PROGRAM_IDS.md` and `../Anchor.toml`.

The file `sanitas_seeker-keypair.json` in this directory is **optional** backup of `target/deploy/sanitas_seeker-keypair.json` after `anchor build`.

- **Do not** commit private keypairs to a public repository.
- If you add a keypair here, keep it out of git (see `.gitignore` in this folder).

The program public key must match `declare_id!` in `programs/fitness-sbt/src/lib.rs` and the table in `../PROGRAM_IDS.md`.
