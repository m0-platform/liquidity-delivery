/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_QUOTER_URL: string
  readonly VITE_NETWORK: 'local' | 'devnet' | 'mainnet'
  readonly VITE_ANVIL_RPC: string
  readonly VITE_SURFPOOL_RPC: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
