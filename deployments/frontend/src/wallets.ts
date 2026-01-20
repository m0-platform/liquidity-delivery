import { createConfig, http } from "@wagmi/core";
import { mainnet, sepolia, baseSepolia } from "@wagmi/core/chains";
import { injected } from "@wagmi/connectors";
import Solflare from "@solflare-wallet/sdk";
import { getEthereumRpc, type NetworkType } from "./config/network";

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
  chains: [mainnet, sepolia, base, baseSepolia],
  connectors: [
    injected({
      shimDisconnect: true,
    }),
  ],
  transports: {
    // Mainnet uses mainnet RPC or falls back to default
    [mainnet.id]: http(
      getEthereumRpc('mainnet') || undefined
    ),
    // Sepolia uses devnet Ethereum RPC
    [sepolia.id]: http(
      getEthereumRpc('devnet') || undefined
    ),
    // Base mainnet
    [base.id]: http(),
    // Base Sepolia (devnet)
    [baseSepolia.id]: http(),
  },
});

// Create a Solflare instance configured for the specified network
export function createSolflare(network: NetworkType): Solflare {
  const solflareNetwork = network === 'mainnet' ? 'mainnet-beta' : 'devnet';
  return new Solflare({ network: solflareNetwork });
}
