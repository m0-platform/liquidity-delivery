import { createConfig, http } from "@wagmi/core";
import { mainnet, sepolia } from "@wagmi/core/chains";
import { injected } from "@wagmi/connectors";
import Solflare from "@solflare-wallet/sdk";

// Define Base chain
const base = {
  id: 8453,
  name: "Base",
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: {
    default: { http: ["https://mainnet.base.org"] },
  },
} as const;

// Wagmi config for EVM wallet connections
// Uses injected connector for browser extension wallets (Rabby, MetaMask, etc.)
export const wagmiConfig = createConfig({
  chains: [mainnet, sepolia, base],
  connectors: [
    injected({
      shimDisconnect: true,
    }),
  ],
  transports: {
    // For local development, use VITE_ANVIL_RPC to point mainnet (chain 1) to local anvil
    [mainnet.id]: http(
      import.meta.env.VITE_ANVIL_RPC || undefined
    ),
    [sepolia.id]: http(),
    // For local development, use VITE_BASE_LOCAL_RPC to point Base (chain 8453) to local anvil
    [base.id]: http(
      import.meta.env.VITE_BASE_LOCAL_RPC || undefined
    ),
  },
});

// Solflare instance (singleton) for Solana wallet connections
export const solflare = new Solflare();
