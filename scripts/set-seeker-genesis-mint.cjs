/**
 * Initialize or rotate the sanitas_seeker **genesis-gate** PDA (which mint `log_workout` / `claim_daily_skr` require).
 *
 * SAFETY: Refuses unless SANITAS_DEVNET_ONLY=1 and RPC looks like devnet/local.
 *
 * Usage (rotate — gate must already exist; signer must be genesis_gate.authority):
 *   SANITAS_DEVNET_ONLY=1 SEEKER_GENESIS_MINT=<base58> node scripts/set-seeker-genesis-mint.cjs
 *
 * First-time init (if PDA does not exist yet):
 *   SANITAS_DEVNET_ONLY=1 INIT_GENESIS_GATE=1 SEEKER_GENESIS_MINT=<base58> node scripts/set-seeker-genesis-mint.cjs
 *
 * Keypair (defaults to Anchor provider wallet path):
 *   WALLET_JSON=$HOME/Groking/projects/sanitas_payer.json
 */

const fs = require('fs');
const path = require('path');
const os = require('os');
const {
  Connection,
  PublicKey,
  Keypair,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  sendAndConfirmTransaction,
} = require('@solana/web3.js');

const PROGRAM_ID = new PublicKey('AwZRzJmcbRx3weqFXUi3MWhaEsS6a7GjvkCJH2DUTkhN');

const IX_INIT_GENESIS_GATE = Buffer.from([132, 181, 190, 37, 239, 132, 237, 243]);
const IX_SET_SEEKER_GENESIS_MINT = Buffer.from([6, 115, 214, 157, 143, 146, 23, 17]);

function assertDevnet(rpcUrl) {
  const u = (rpcUrl || '').toLowerCase();
  const ok =
    u.includes('devnet') ||
    u.includes('dev.') ||
    u === 'https://api.devnet.solana.com' ||
    u.startsWith('http://127.') ||
    u.startsWith('http://localhost');
  if (!ok) {
    console.error('Refusing to run: RPC_URL does not look like devnet/local:', rpcUrl);
    process.exit(1);
  }
}

function loadKeypair(p) {
  const raw = JSON.parse(fs.readFileSync(p, 'utf8'));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

function encodePubkeyArg(pk) {
  const out = Buffer.alloc(8 + 32);
  pk.toBuffer().copy(out, 8);
  return out;
}

async function main() {
  if (process.env.SANITAS_DEVNET_ONLY !== '1') {
    console.error('Set SANITAS_DEVNET_ONLY=1 to run.');
    process.exit(1);
  }

  const mintStr = process.env.SEEKER_GENESIS_MINT;
  if (!mintStr) {
    console.error('Set SEEKER_GENESIS_MINT=<base58 pubkey> (Token-2022 mint for this cluster).');
    process.exit(1);
  }

  const rpcUrl = process.env.RPC_URL || 'https://api.devnet.solana.com';
  assertDevnet(rpcUrl);

  const home = os.homedir();
  const walletPath =
    process.env.WALLET_JSON ||
    path.join(home, 'Groking', 'projects', 'sanitas_payer.json');
  if (!fs.existsSync(walletPath)) {
    console.error('Wallet not found:', walletPath, '(set WALLET_JSON=...)');
    process.exit(1);
  }

  const payer = loadKeypair(walletPath);
  const seekerMint = new PublicKey(mintStr);
  const connection = new Connection(rpcUrl, 'confirmed');

  const [genesisGate] = PublicKey.findProgramAddressSync([Buffer.from('genesis-gate')], PROGRAM_ID);

  const init = process.env.INIT_GENESIS_GATE === '1';
  const data = Buffer.alloc(8 + 32);
  if (init) {
    IX_INIT_GENESIS_GATE.copy(data, 0);
  } else {
    IX_SET_SEEKER_GENESIS_MINT.copy(data, 0);
  }
  seekerMint.toBuffer().copy(data, 8);

  let ix;
  if (init) {
    ix = new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        { pubkey: genesisGate, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    });
  } else {
    ix = new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        { pubkey: genesisGate, isSigner: false, isWritable: true },
        { pubkey: payer.publicKey, isSigner: true, isWritable: false },
      ],
      data,
    });
  }

  const tx = new Transaction().add(ix);
  const sig = await sendAndConfirmTransaction(connection, tx, [payer], {
    commitment: 'confirmed',
  });
  console.log(init ? 'initialize_genesis_gate' : 'set_seeker_genesis_mint', 'ok', sig);
  console.log('Genesis gate PDA:', genesisGate.toBase58());
  console.log('Seeker genesis mint set to:', seekerMint.toBase58());
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
