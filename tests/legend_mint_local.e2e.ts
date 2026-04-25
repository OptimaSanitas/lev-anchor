/**
 * Local-validator e2e for merged `mint_sbt` (free cohort) + on-chain counters.
 * Runs before `sanitas_seeker.ts` (lexicographic order). Requires `tests/fixtures/dcrf_skr_mint.json` + `[[test.validator.account]]` in Anchor.toml.
 *
 * `extend_legend` needs ≥1 SKR in the user ATA (fixture mint has no local mint authority) — exercise on devnet (see docs/LEGEND_MINT_RESTORE_AND_TEST.md).
 */
import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  createMint,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { sanitas_seeker } from "../target/types/sanitas_seeker";

const SKR_MINT = new PublicKey("DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ");
const DEV_FEE_AUTHORITY = new PublicKey(
  "B9Qo6q398kvryKQuCUMjRxQHMbVTGTc3wwSbrRoKaTrc",
);

function isLocalValidator(connection: anchor.web3.Connection): boolean {
  const u = connection.rpcEndpoint;
  return u.includes("127.0.0.1") || u.includes("localhost");
}

async function tryRpc<T>(label: string, fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (e: unknown) {
    const msg = String((e as Error)?.message ?? e);
    if (msg.includes("already in use") || msg.includes("0x0")) {
      console.log(`ℹ️ ${label} — already initialized`);
      throw Object.assign(new Error("SKIP_OK"), { code: "SKIP_OK" });
    }
    throw e;
  }
}

describe("legend_mint_local_e2e", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace.sanitas_seeker as Program<sanitas_seeker>;
  const payer = provider.wallet as anchor.Wallet;
  const conn = provider.connection;

  before(async function () {
    this.timeout(60_000);
    if (!isLocalValidator(conn)) return;
    const b = await conn.getBalance(payer.publicKey);
    if (b < 2e9) {
      const sig = await conn.requestAirdrop(payer.publicKey, 5e9);
      const latest = await conn.getLatestBlockhash("confirmed");
      await conn.confirmTransaction(
        { signature: sig, ...latest },
        "confirmed",
      );
    }
  });

  it("mint_sbt (free cohort): lab genesis mint + SKR ATAs + legend_supply bump", async function () {
    if (!isLocalValidator(conn)) {
      console.log("skip: not local validator (legend e2e uses cloned devnet SKR mint)");
      this.skip();
    }
    this.timeout(180_000);

    // Legacy SPL mint (still satisfies on-chain `token_interface` genesis checks).
    const labGenesisMint = await createMint(
      conn,
      payer.payer,
      payer.publicKey,
      null,
      0,
      undefined,
      { commitment: "confirmed" },
      TOKEN_PROGRAM_ID,
    );

    // Separate minter from `payer` (wallet is also hardcoded `dev_fee_authority` → same SKR ATA would duplicate mut accounts).
    const minter = Keypair.generate();
    const minterGenesisAta = await getOrCreateAssociatedTokenAccount(
      conn,
      payer.payer,
      labGenesisMint,
      minter.publicKey,
      false,
      "confirmed",
      { commitment: "confirmed" },
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    await mintTo(
      conn,
      payer.payer,
      labGenesisMint,
      minterGenesisAta.address,
      payer.publicKey,
      1n,
      [],
      { commitment: "confirmed" },
      TOKEN_PROGRAM_ID,
    );

    const mb = await conn.getBalance(minter.publicKey);
    if (mb < 2e9) {
      const sig = await conn.requestAirdrop(minter.publicKey, 5e9);
      const latest = await conn.getLatestBlockhash("confirmed");
      await conn.confirmTransaction(
        { signature: sig, ...latest },
        "confirmed",
      );
    }

    try {
      await tryRpc("initializeMintConfig", () =>
        program.methods.initializeMintConfig().rpc(),
      );
    } catch (e: unknown) {
      if ((e as { code?: string }).code === "SKIP_OK") {
        /* ok */
      } else {
        throw e;
      }
    }

    try {
      await tryRpc("initializeRewardPools", () =>
        program.methods.initializeRewardPools().rpc(),
      );
    } catch (e: unknown) {
      if ((e as { code?: string }).code === "SKIP_OK") {
        /* ok */
      } else {
        throw e;
      }
    }

    try {
      await tryRpc("initializeLegendSupply", () =>
        program.methods.initializeLegendSupply().rpc(),
      );
    } catch (e: unknown) {
      if ((e as { code?: string }).code === "SKIP_OK") {
        /* ok */
      } else {
        throw e;
      }
    }

    try {
      await tryRpc("initializeGenesisGate", () =>
        program.methods.initializeGenesisGate(labGenesisMint).rpc(),
      );
    } catch (e: unknown) {
      if ((e as { code?: string }).code === "SKIP_OK") {
        await program.methods
          .setSeekerGenesisMint(labGenesisMint)
          .accounts({ user: payer.publicKey })
          .rpc();
      } else {
        throw e;
      }
    }

    try {
      await tryRpc("initializeStakeVault", () =>
        program.methods.initializeStakeVault().rpc(),
      );
    } catch (e: unknown) {
      if ((e as { code?: string }).code === "SKIP_OK") {
        /* ok */
      } else {
        throw e;
      }
    }

    const [rewardPool] = PublicKey.findProgramAddressSync(
      [Buffer.from("reward-pool")],
      program.programId,
    );
    const [stakeVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("stake-vault")],
      program.programId,
    );

    const rewardPoolAta = getAssociatedTokenAddressSync(
      SKR_MINT,
      rewardPool,
      true,
    );
    const devFeeAta = getAssociatedTokenAddressSync(
      SKR_MINT,
      DEV_FEE_AUTHORITY,
      false,
    );
    const minterSkrAta = getAssociatedTokenAddressSync(
      SKR_MINT,
      minter.publicKey,
      false,
    );

    const ixs = [];
    for (const ataOwner of [
      [rewardPoolAta, rewardPool] as const,
      [devFeeAta, DEV_FEE_AUTHORITY] as const,
    ]) {
      const [ata, owner] = ataOwner;
      const info = await conn.getAccountInfo(ata);
      if (!info) {
        ixs.push(
          createAssociatedTokenAccountInstruction(
            payer.publicKey,
            ata,
            owner,
            SKR_MINT,
            TOKEN_PROGRAM_ID,
            ASSOCIATED_TOKEN_PROGRAM_ID,
          ),
        );
      }
    }
    if (!(await conn.getAccountInfo(minterSkrAta))) {
      ixs.push(
        createAssociatedTokenAccountInstruction(
          payer.publicKey,
          minterSkrAta,
          minter.publicKey,
          SKR_MINT,
          TOKEN_PROGRAM_ID,
          ASSOCIATED_TOKEN_PROGRAM_ID,
        ),
      );
    }
    if (ixs.length) {
      const tx = new Transaction().add(...ixs);
      await provider.sendAndConfirm(tx, [payer.payer]);
    }

    const ex = "sprint-interval";
    const vid = "video-e2e";
    const label1 = "Sprint Interval";
    const label2 = "E2E";

    const [sbtPda] = PublicKey.findProgramAddressSync(
      [minter.publicKey.toBuffer(), Buffer.from("user-sbt"), Buffer.from(ex)],
      program.programId,
    );
    const [userExercisePda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("user-exercise"),
        minter.publicKey.toBuffer(),
        Buffer.from(ex),
      ],
      program.programId,
    );
    const [legendEntitlementPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("legend-entitlement"),
        minter.publicKey.toBuffer(),
        Buffer.from(ex),
      ],
      program.programId,
    );
    const [legendSalePda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("legend-sale"),
        minter.publicKey.toBuffer(),
        Buffer.from(ex),
      ],
      program.programId,
    );

    const mintIx = await program.methods
      .mintSbt(ex, vid, label1, label2)
      .accounts({
        user: minter.publicKey,
        genesisMint: labGenesisMint,
        userGenesisAta: minterGenesisAta.address,
        devFeeAuthority: DEV_FEE_AUTHORITY,
        skrMint: SKR_MINT,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    const recordIx = await program.methods
      .recordLegendEntitlement(ex)
      .accounts({
        user: minter.publicKey,
        sbt: sbtPda,
        userExercise: userExercisePda,
        legendEntitlement: legendEntitlementPda,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    const tx = new Transaction().add(mintIx, recordIx);
    await provider.sendAndConfirm(tx, [minter], { commitment: "confirmed" });
    const sbt = await program.account.sbtAccount.fetch(sbtPda);
    assert.ok(sbt.owner.equals(minter.publicKey));
    assert.equal(sbt.label1, label1);
    assert.equal(sbt.label2, label2);
    assert.equal(sbt.isEarly, true);

    const mintCfg = await program.account.mintConfig.fetch(
      PublicKey.findProgramAddressSync(
        [Buffer.from("mint-config")],
        program.programId,
      )[0],
    );
    assert.equal(mintCfg.mintedPhase1, 1);

    const legend = await program.account.legendSupply.fetch(
      PublicKey.findProgramAddressSync(
        [Buffer.from("legend-supply")],
        program.programId,
      )[0],
    );
    assert.equal(legend.minted, 1);

    const rem = await program.methods
      .getLegendMintRemaining()
      .accounts({
        legendSupply: PublicKey.findProgramAddressSync(
          [Buffer.from("legend-supply")],
          program.programId,
        )[0],
      })
      .view();
    assert.equal(rem, 10_000 - 1);

    const ent = await program.account.legendEntitlement.fetch(legendEntitlementPda);
    assert.ok(ent.visualOwner.equals(minter.publicKey));
    assert.ok(ent.statusHolder.equals(minter.publicKey));

    await program.methods
      .listLegendSale(ex, new BN(1_000_000))
      .accounts({
        visualOwner: minter.publicKey,
        legendEntitlement: legendEntitlementPda,
        legendSale: legendSalePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([minter])
      .rpc();

    const listed = await program.account.legendSale.fetch(legendSalePda);
    assert.equal(listed.state, 1);
    assert.equal(listed.priceLamports.toNumber(), 1_000_000);

    await program.methods
      .cancelLegendSale(ex)
      .accounts({
        visualOwner: minter.publicKey,
        legendSale: legendSalePda,
      })
      .signers([minter])
      .rpc();

    const cancelled = await program.account.legendSale.fetch(legendSalePda);
    assert.equal(cancelled.state, 3);
  });
});
