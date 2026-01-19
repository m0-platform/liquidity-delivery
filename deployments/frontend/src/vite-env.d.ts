/// <reference types="vite/client" />

import { Buffer } from 'buffer'

declare global {
  interface Window {
    Buffer: typeof Buffer
  }
}

interface ImportMetaEnv {
  readonly VITE_QUOTER_URL: string
  readonly VITE_SOLVER_URL: string
  readonly VITE_MOCK_API_URL: string
  readonly VITE_NETWORK: 'local' | 'devnet' | 'mainnet'
  readonly VITE_ANVIL_RPC: string
  readonly VITE_SURFPOOL_RPC: string
  // Reown AppKit (required for devnet/mainnet)
  readonly VITE_REOWN_PROJECT_ID: string
  // Local development private keys (only used when VITE_NETWORK=local)
  readonly VITE_LOCAL_EVM_PRIVATE_KEY: string
  readonly VITE_LOCAL_SVM_PRIVATE_KEY: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
