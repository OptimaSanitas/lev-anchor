/**
 * Submits 3x approve_upgrade for the upgrade governor 3-of-5 gate (signer indices 0, 1, 3)
 * using the local test keypairs: sanitas_signer1, sanitas_signer2, sanitas_signer3.
 *
 * SAFETY: Only runs when explicitly allowed and the RPC URL is a devnet endpoint.
 * Does nothing on mainnet.
 *
 * Usage:
 *   SANITAS_DEVNET_ONLY=1 node scripts/devnet-auto-approve-gate.cjs
 *   RPC_URL=https://api.devnet.solana.com SANITAS_DEVNET_ONLY=1 node scripts/devnet-auto-approve-gate.cjs
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
  sendAndConfirmTransaction,
} = require('@solana/web3.js');

const GOVERNOR_PROGRAM_ID = new PublicKey('73yx8W1HB7kooLXSgoqJtNoBNbRUKaotFNygd7b9dDRQ');

/** Must match on-chain `UPGRADE_SIGNERS` order in `programs/governor/src/lib.rs` */
const EXPECTED_SIGNERS = [
  '5fkgfLSGCxJTWcqQHfzigQUnxA1NAaCmmCjQbXmTvVzc',
  'CwyNHESJ95mccZkGPEEApQdeB4XEV5mSL1SRkn6Ee8qG',
  '8TeEjQkh2CQTbKo57r3n5GrYGYUzvrmbj1eRJgbjZsjp',
  'BpDZ6jrcPYo1GoM4DWk857ys4R7MgyZb4FmHjkC9beuH',
  'CQsV3Wj6pdcgEkk5hkS6bd31Q2xp9fCuAqvV9WoLjqAR',
];

const APPROVE_DISCRIMINATOR = Buffer.from([218, 6, 0, 77, 31, 202, 206, 123]);

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

async function main() {
  if (process.env.SANITAS_DEVNET_ONLY !== '1') {
    console.error('Set SANITAS_DEVNET_ONLY=1 to run (devnet-only guard).');
    process.exit(1);
  }

  const rpcUrl = process.env.RPC_URL || 'https://api.devnet.solana.com';
  assertDevnet(rpcUrl);

  const home = os.homedir();
  const projects = path.join(home, 'Groking', 'projects');
  const jobs = [
    { file: 'sanitas_signer1.json', index: 0 },
    { file: 'sanitas_signer2.json', index: 1 },
    { file: 'sanitas_signer3.json', index: 3 },
  ];

  const connection = new Connection(rpcUrl, 'confirmed');

  const [gatePda] = PublicKey.findProgramAddressSync(
    [Buffer.from('upgrade-gate')],
    GOVERNOR_PROGRAM_ID
  );

  for (const { file, index } of jobs) {
    const kpPath = path.join(projects, file);
    if (!fs.existsSync(kpPath)) {
      console.error('Missing keypair:', kpPath);
      process.exit(1);
    }
    const signer = loadKeypair(kpPath);
    const expected = EXPECTED_SIGNERS[index];
    if (signer.publicKey.toBase58() !== expected) {
      console.error(
        `Keypair ${file} pubkey ${signer.publicKey.toBase58()} does not match UPGRADE_SIGNERS[${index}] ${expected}`
      );
      process.exit(1);
    }

    const data = Buffer.alloc(9);
    APPROVE_DISCRIMINATOR.copy(data, 0);
    data[8] = index;

    const ix = new TransactionInstruction({
      keys: [
        { pubkey: gatePda, isSigner: false, isWritable: true },
        { pubkey: signer.publicKey, isSigner: true, isWritable: false },
      ],
      programId: GOVERNOR_PROGRAM_ID,
      data,
    });

    const tx = new Transaction().add(ix);
    const sig = await sendAndConfirmTransaction(connection, tx, [signer], {
      commitment: 'confirmed',
    });
    console.log(`OK approve signer ${index} (${file})`, sig);
  }

  console.log('Done. Gate should have 3 approvals (bits 0,1,3). Run check in admin-final.html or checkGateStatus.');
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
