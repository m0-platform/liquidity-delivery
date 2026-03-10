import { keccak256 } from "js-sha3";

/**
 * OrderData matches the cross-chain canonical encoding from
 * svm/programs/order_book/src/state/orders.rs
 */
export interface OrderData {
  version: number; // u16
  sender: Uint8Array; // 32 bytes
  nonce: bigint; // u64
  originChainId: number; // u32
  destChainId: number; // u32
  createdAt: bigint; // u64
  fillDeadline: bigint; // u64
  tokenIn: Uint8Array; // 32 bytes
  tokenOut: Uint8Array; // 32 bytes
  amountIn: bigint; // u128
  amountOut: bigint; // u128
  recipient: Uint8Array; // 32 bytes
  solver: Uint8Array; // 32 bytes
}

// Total encoded size: 2 + 32 + 8 + 4 + 4 + 8 + 8 + 32 + 32 + 16 + 16 + 32 + 32 = 226
const ENCODED_SIZE = 226;

function writeBE(buf: Buffer, offset: number, value: bigint, bytes: number): void {
  for (let i = bytes - 1; i >= 0; i--) {
    buf[offset + i] = Number(value & 0xffn);
    value >>= 8n;
  }
}

/**
 * Encode OrderData into a packed byte array using big-endian encoding.
 * This must match the Rust/EVM encoding exactly.
 */
export function encodeOrderData(order: OrderData): Buffer {
  const buf = Buffer.alloc(ENCODED_SIZE);
  let offset = 0;

  // version: u16 BE
  buf.writeUInt16BE(order.version, offset);
  offset += 2;

  // sender: 32 bytes
  Buffer.from(order.sender).copy(buf, offset);
  offset += 32;

  // nonce: u64 BE
  writeBE(buf, offset, order.nonce, 8);
  offset += 8;

  // originChainId: u32 BE
  buf.writeUInt32BE(order.originChainId, offset);
  offset += 4;

  // destChainId: u32 BE
  buf.writeUInt32BE(order.destChainId, offset);
  offset += 4;

  // createdAt: u64 BE
  writeBE(buf, offset, order.createdAt, 8);
  offset += 8;

  // fillDeadline: u64 BE
  writeBE(buf, offset, order.fillDeadline, 8);
  offset += 8;

  // tokenIn: 32 bytes
  Buffer.from(order.tokenIn).copy(buf, offset);
  offset += 32;

  // tokenOut: 32 bytes
  Buffer.from(order.tokenOut).copy(buf, offset);
  offset += 32;

  // amountIn: u128 BE
  writeBE(buf, offset, order.amountIn, 16);
  offset += 16;

  // amountOut: u128 BE
  writeBE(buf, offset, order.amountOut, 16);
  offset += 16;

  // recipient: 32 bytes
  Buffer.from(order.recipient).copy(buf, offset);
  offset += 32;

  // solver: 32 bytes
  Buffer.from(order.solver).copy(buf, offset);
  offset += 32;

  return buf;
}

/**
 * Compute order ID as keccak256 hash of encoded OrderData.
 * Returns 32-byte Uint8Array.
 */
export function computeOrderId(order: OrderData): Uint8Array {
  const encoded = encodeOrderData(order);
  return new Uint8Array(keccak256.arrayBuffer(encoded));
}
