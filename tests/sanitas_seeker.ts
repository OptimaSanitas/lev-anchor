import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { sanitas_seeker } from "../target/types/sanitas_seeker";

describe("sanitas_seeker", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.sanitas_seeker as Program<sanitas_seeker>;

  before(async function () {
    this.timeout(60_000);
    const provider = anchor.getProvider() as anchor.AnchorProvider;
    const conn = provider.connection;
    const w = provider.wallet.publicKey;
    if (
      !conn.rpcEndpoint.includes("127.0.0.1") &&
      !conn.rpcEndpoint.includes("localhost")
    ) {
      return;
    }
    const b = await conn.getBalance(w);
    if (b < 2e9) {
      const sig = await conn.requestAirdrop(w, 5e9);
      const latest = await conn.getLatestBlockhash("confirmed");
      await conn.confirmTransaction(
        { signature: sig, ...latest },
        "confirmed",
      );
    }
  });

  it("Initializes Mint Config", async () => {
    try {
      const tx = await program.methods.initializeMintConfig().rpc();
      console.log("✅ Mint Config initialized — Tx signature:", tx);
    } catch (e: unknown) {
      const msg = String((e as Error)?.message ?? e);
      if (msg.includes("already in use") || msg.includes("0x0")) {
        console.log("ℹ️ Mint config PDA already initialized — skipping");
        return;
      }
      throw e;
    }
  });

  it("Initializes Legend Supply PDA", async () => {
    try {
      const tx = await program.methods.initializeLegendSupply().rpc();
      console.log("✅ Legend supply initialized — Tx signature:", tx);
    } catch (e: unknown) {
      const msg = String((e as Error)?.message ?? e);
      if (msg.includes("already in use") || msg.includes("0x0")) {
        console.log("ℹ️ Legend supply PDA already initialized — skipping");
        return;
      }
      throw e;
    }
  });

  it("Initializes Genesis Gate", async () => {
    const seekerGenesis = new anchor.web3.PublicKey(
      "GT22s89nU4iWFkNXj1Bw6uYhJJWDRPpShHt4Bk8f99Te"
    );
    try {
      const tx = await program.methods
        .initializeGenesisGate(seekerGenesis)
        .rpc();
      console.log("✅ Genesis gate initialized — Tx signature:", tx);
    } catch (e: unknown) {
      const msg = String((e as Error)?.message ?? e);
      if (msg.includes("already in use") || msg.includes("0x0")) {
        console.log("ℹ️ Genesis gate PDA already initialized — skipping");
        return;
      }
      throw e;
    }
  });

  it("Initializes Treasury Policy PDA", async () => {
    const auth = anchor.getProvider().publicKey!;
    const distributeBps = 2500;
    const minChangeIntervalSec = new anchor.BN(0);
    try {
      const tx = await program.methods
        .initializeTreasuryPolicy(auth, distributeBps, minChangeIntervalSec)
        .rpc();
      console.log("✅ Treasury policy initialized — Tx signature:", tx);
    } catch (e: unknown) {
      const msg = String((e as Error)?.message ?? e);
      if (msg.includes("already in use") || msg.includes("0x0")) {
        console.log("ℹ️ Treasury policy PDA already initialized — skipping");
        return;
      }
      throw e;
    }
  });
});