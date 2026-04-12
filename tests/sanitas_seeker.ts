import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { sanitas_seeker } from "../target/types/sanitas_seeker";

describe("sanitas_seeker", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.sanitas_seeker as Program<sanitas_seeker>;

  it("Initializes Mint Config", async () => {
    const tx = await program.methods
      .initializeMintConfig()
      .rpc();

    console.log("✅ Mint Config initialized — Tx signature:", tx);
  });
});