import { AnchorProvider, Program, setProvider, BN } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getMint,
} from "@solana/spl-token";
import * as fs from "fs";
import { computeOrderId, OrderData } from "./lib/order-id";

// Load IDL from build output
import orderBookIdl from "../target/idl/order_book.json";

const PROGRAM_ID = new PublicKey("MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK");

// PDA seeds
const GLOBAL_SEED = Buffer.from("global");
const NONCE_SEED_PREFIX = Buffer.from("nonce");
const ORDER_SEED_PREFIX = Buffer.from("order");
const DESTINATION_SEED_PREFIX = Buffer.from("destination");
const EVENT_AUTHORITY_SEED = Buffer.from("__event_authority");

// --- CLI argument parsing ---

interface CliArgs {
  tokenIn: string;
  amountIn: string;
  destChainId: number;
  tokenOut: string; // hex bytes32
  amountOut: string;
  recipient: string; // hex bytes32
  solver: string; // hex bytes32
  deadlineOffset: number; // seconds
  keypairPath: string;
  rpcUrl: string;
  dryRun: boolean;
}

function parseArgs(): CliArgs {
  const args = process.argv.slice(2);
  const parsed: Partial<CliArgs> = {
    recipient: "0".repeat(64),
    solver: "0".repeat(64),
    deadlineOffset: 3600,
    dryRun: false,
  };

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--token-in":
        parsed.tokenIn = args[++i];
        break;
      case "--amount-in":
        parsed.amountIn = args[++i];
        break;
      case "--dest-chain-id":
        parsed.destChainId = parseInt(args[++i]);
        break;
      case "--token-out":
        parsed.tokenOut = args[++i].replace(/^0x/, "").padStart(64, "0");
        break;
      case "--amount-out":
        parsed.amountOut = args[++i];
        break;
      case "--recipient":
        parsed.recipient = args[++i].replace(/^0x/, "").padStart(64, "0");
        break;
      case "--solver":
        parsed.solver = args[++i].replace(/^0x/, "").padStart(64, "0");
        break;
      case "--deadline":
        parsed.deadlineOffset = parseInt(args[++i]);
        break;
      case "--keypair":
        parsed.keypairPath = args[++i];
        break;
      case "--rpc-url":
        parsed.rpcUrl = args[++i];
        break;
      case "--dry-run":
        parsed.dryRun = true;
        break;
      default:
        console.error(`Unknown argument: ${args[i]}`);
        process.exit(1);
    }
  }

  // Validate required args
  const required: (keyof CliArgs)[] = [
    "tokenIn",
    "amountIn",
    "destChainId",
    "tokenOut",
    "amountOut",
    "keypairPath",
    "rpcUrl",
  ];
  for (const key of required) {
    if (parsed[key] === undefined || parsed[key] === null) {
      console.error(`Missing required argument: --${key.replace(/([A-Z])/g, "-$1").toLowerCase()}`);
      process.exit(1);
    }
  }

  return parsed as CliArgs;
}

// --- Helpers ---

function hexToBytes32(hex: string): number[] {
  const bytes = Buffer.from(hex, "hex");
  if (bytes.length !== 32) throw new Error(`Expected 32 bytes, got ${bytes.length}`);
  return Array.from(bytes);
}

function loadKeypair(path: string): Keypair {
  const raw = JSON.parse(fs.readFileSync(path, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

async function detectTokenProgram(
  connection: Connection,
  mint: PublicKey
): Promise<PublicKey> {
  const info = await connection.getAccountInfo(mint);
  if (!info) throw new Error(`Mint account not found: ${mint.toBase58()}`);
  if (info.owner.equals(TOKEN_2022_PROGRAM_ID)) return TOKEN_2022_PROGRAM_ID;
  return TOKEN_PROGRAM_ID;
}

// --- Main ---

async function main() {
  const args = parseArgs();

  // Load keypair and connect
  const payer = loadKeypair(args.keypairPath);
  const connection = new Connection(args.rpcUrl, "confirmed");

  console.log(`Payer:    ${payer.publicKey.toBase58()}`);
  console.log(`RPC:      ${args.rpcUrl.replace(/\/\/.*@/, "//***@")}`); // hide API key
  console.log(`Dry run:  ${args.dryRun}`);
  console.log();

  // Set up Anchor provider (wallet adapter for signing)
  const provider = new AnchorProvider(
    connection,
    {
      publicKey: payer.publicKey,
      signTransaction: async (tx) => {
        tx.sign(payer);
        return tx;
      },
      signAllTransactions: async (txs) => {
        txs.forEach((tx) => tx.sign(payer));
        return txs;
      },
    },
    { commitment: "confirmed" }
  );
  setProvider(provider);

  const program = new Program(orderBookIdl as any, provider);

  // --- Derive PDAs ---

  const [globalAccount] = PublicKey.findProgramAddressSync(
    [GLOBAL_SEED],
    PROGRAM_ID
  );

  // Fetch global account for chain_id
  const globalData = await (program.account as any).orderBookGlobal.fetch(globalAccount);
  const originChainId: number = globalData.chainId;
  console.log(`Origin chain ID: ${originChainId}`);

  // Fetch sender nonce (default to 0 if account doesn't exist)
  const [senderNonceAccount] = PublicKey.findProgramAddressSync(
    [NONCE_SEED_PREFIX, payer.publicKey.toBuffer()],
    PROGRAM_ID
  );

  let nonce = 0n;
  try {
    const nonceData = await (program.account as any).nonce.fetch(senderNonceAccount);
    nonce = BigInt(nonceData.value.toString());
    console.log(`Sender nonce: ${nonce}`);
  } catch {
    console.log("Sender nonce account not found — using nonce 0 (will be created)");
  }

  // Timestamps
  const now = BigInt(Math.floor(Date.now() / 1000));
  const createdAt = now + 30n; // 30 second buffer
  const fillDeadline = createdAt + BigInt(args.deadlineOffset);

  // Token setup
  const tokenInMint = new PublicKey(args.tokenIn);
  const tokenInProgram = await detectTokenProgram(connection, tokenInMint);
  console.log(`Token program: ${tokenInProgram.equals(TOKEN_2022_PROGRAM_ID) ? "Token-2022" : "Token"}`);

  const senderTokenInAccount = getAssociatedTokenAddressSync(
    tokenInMint,
    payer.publicKey,
    false,
    tokenInProgram
  );

  // Build OrderData for ID computation
  const orderData: OrderData = {
    version: 1,
    sender: payer.publicKey.toBytes(),
    nonce,
    originChainId,
    destChainId: args.destChainId,
    createdAt,
    fillDeadline,
    tokenIn: tokenInMint.toBytes(),
    tokenOut: new Uint8Array(Buffer.from(args.tokenOut, "hex")),
    amountIn: BigInt(args.amountIn),
    amountOut: BigInt(args.amountOut),
    recipient: new Uint8Array(Buffer.from(args.recipient, "hex")),
    solver: new Uint8Array(Buffer.from(args.solver, "hex")),
  };

  const orderId = computeOrderId(orderData);
  const orderIdHex = Buffer.from(orderId).toString("hex");
  console.log(`\nOrder ID: 0x${orderIdHex}`);

  // Derive order PDA
  const [orderPda] = PublicKey.findProgramAddressSync(
    [ORDER_SEED_PREFIX, Buffer.from(orderId)],
    PROGRAM_ID
  );
  console.log(`Order PDA: ${orderPda.toBase58()}`);

  // Order token ATA
  const orderTokenInAta = getAssociatedTokenAddressSync(
    tokenInMint,
    orderPda,
    true, // allowOwnerOffCurve for PDA
    tokenInProgram
  );

  // Destination PDA (little-endian chain ID bytes)
  const destChainIdBuf = Buffer.alloc(4);
  destChainIdBuf.writeUInt32LE(args.destChainId);
  const [destinationAccount] = PublicKey.findProgramAddressSync(
    [DESTINATION_SEED_PREFIX, destChainIdBuf],
    PROGRAM_ID
  );

  // Event authority PDA
  const [eventAuthority] = PublicKey.findProgramAddressSync(
    [EVENT_AUTHORITY_SEED],
    PROGRAM_ID
  );

  // Build instruction params
  const params = {
    destChainId: args.destChainId,
    createdAt: new BN(createdAt.toString()),
    fillDeadline: new BN(fillDeadline.toString()),
    tokenOut: hexToBytes32(args.tokenOut),
    amountIn: new BN(args.amountIn),
    amountOut: new BN(args.amountOut),
    recipient: hexToBytes32(args.recipient),
    solver: hexToBytes32(args.solver),
  };

  console.log("\n--- Order Details ---");
  console.log(`Token In:        ${tokenInMint.toBase58()}`);
  console.log(`Amount In:       ${args.amountIn}`);
  console.log(`Dest Chain ID:   ${args.destChainId}`);
  console.log(`Token Out:       0x${args.tokenOut}`);
  console.log(`Amount Out:      ${args.amountOut}`);
  console.log(`Recipient:       0x${args.recipient}`);
  console.log(`Solver:          0x${args.solver}`);
  console.log(`Created At:      ${createdAt} (now + 30s)`);
  console.log(`Fill Deadline:   ${fillDeadline} (created_at + ${args.deadlineOffset}s)`);
  console.log();

  // Build the transaction
  const tx = await program.methods
    .openOrder(params)
    .accountsPartial({
      payer: payer.publicKey,
      tokenAuthority: null,
      globalAccount,
      destinationAccount: args.destChainId !== originChainId ? destinationAccount : null,
      tokenInMint,
      senderTokenInAccount,
      senderNonceAccount,
      order: orderPda,
      orderTokenInAta,
      tokenInProgram,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      eventAuthority,
      program: PROGRAM_ID,
    })
    .transaction();

  tx.feePayer = payer.publicKey;
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

  if (args.dryRun) {
    console.log("--- DRY RUN: Simulating transaction ---");
    const simulation = await connection.simulateTransaction(tx);
    if (simulation.value.err) {
      console.error("Simulation FAILED:", JSON.stringify(simulation.value.err, null, 2));
      if (simulation.value.logs) {
        console.error("\nLogs:");
        simulation.value.logs.forEach((log) => console.error(`  ${log}`));
      }
      process.exit(1);
    }
    console.log("Simulation SUCCESS");
    if (simulation.value.logs) {
      simulation.value.logs.forEach((log) => console.log(`  ${log}`));
    }
    console.log(`\nUnits consumed: ${simulation.value.unitsConsumed}`);
  } else {
    console.log("--- Sending transaction ---");
    tx.sign(payer);
    const sig = await connection.sendRawTransaction(tx.serialize(), {
      skipPreflight: false,
    });
    console.log(`Transaction sent: ${sig}`);

    const confirmation = await connection.confirmTransaction(sig, "confirmed");
    if (confirmation.value.err) {
      console.error("Transaction FAILED:", JSON.stringify(confirmation.value.err));
      process.exit(1);
    }

    console.log(`\nOrder created successfully!`);
    console.log(`  Order ID:  0x${orderIdHex}`);
    console.log(`  Order PDA: ${orderPda.toBase58()}`);
    console.log(`  Tx:        ${sig}`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
