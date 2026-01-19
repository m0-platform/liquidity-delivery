/// <reference types="vite/client" />

import { Buffer } from "buffer";

declare global {
  interface Window {
    Buffer: typeof Buffer;
  }
}

interface ImportMetaEnv {
  // Quoter API URLs per network
  readonly VITE_QUOTER_URL_LOCAL: string;
  readonly VITE_QUOTER_URL_DEVNET: string;
  readonly VITE_QUOTER_URL_MAINNET: string;

  readonly VITE_LIQUIDITY_API_LOCAL: string;
  readonly VITE_LIQUIDITY_API_DEVNET: string;
  readonly VITE_LIQUIDITY_API_MAINNET: string;

  readonly VITE_ORDERS_API_LOCAL: string;
  readonly VITE_ORDERS_API_DEVNET: string;
  readonly VITE_ORDERS_API_MAINNET: string;

  // Localnet RPC endpoints
  readonly VITE_LOCALNET_ETHEREUM_RPC: string;
  readonly VITE_LOCALNET_SOLANA_RPC: string;
  readonly VITE_LOCALNET_BASE_RPC: string;

  // Devnet RPC endpoints
  readonly VITE_DEVNET_ETHEREUM_RPC: string;
  readonly VITE_DEVNET_SOLANA_RPC: string;

  // Mainnet RPC endpoints
  readonly VITE_MAINNET_ETHEREUM_RPC: string;
  readonly VITE_MAINNET_SOLANA_RPC: string;

  // Reown AppKit (required for devnet/mainnet)
  readonly VITE_REOWN_PROJECT_ID: string;

  // Local development keys
  readonly VITE_LOCAL_EVM_PRIVATE_KEY: string;
  readonly VITE_LOCAL_SVM_PRIVATE_KEY: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
