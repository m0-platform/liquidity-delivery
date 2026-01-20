import { ref } from "vue";
import { JsonRpcProvider, Contract } from "ethers";
import { Connection, PublicKey } from "@solana/web3.js";
import { ASSOCIATED_TOKEN_PROGRAM_ID, Token } from "@solana/spl-token";

// Standard ERC20 ABI for balanceOf
const ERC20_ABI = [
  "function balanceOf(address owner) view returns (uint256)",
  "function decimals() view returns (uint8)",
];

export interface BalanceResult {
  raw: string;
  formatted: string;
  decimals: number;
}

export function useBalance() {
  const loading = ref(false);
  const error = ref<string | null>(null);

  // Fetch ERC20 token balance on EVM chain
  async function fetchEvmBalance(
    rpcUrl: string,
    walletAddress: string,
    tokenAddress: string,
    decimals: number = 6,
  ): Promise<BalanceResult | null> {
    loading.value = true;
    error.value = null;

    try {
      const provider = new JsonRpcProvider(rpcUrl);
      const contract = new Contract(tokenAddress, ERC20_ABI, provider);

      const balance = await contract.balanceOf(walletAddress);
      const balanceStr = balance.toString();

      // Format balance with decimals
      const formatted = formatBalance(balanceStr, decimals);

      return {
        raw: balanceStr,
        formatted,
        decimals,
      };
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : "Failed to fetch EVM balance";
      console.error("EVM balance fetch error:", err);
      return null;
    } finally {
      loading.value = false;
    }
  }

  // Fetch SPL token balance on Solana
  async function fetchSolanaBalance(
    rpcUrl: string,
    walletAddress: string,
    mintAddress: string,
    tokenProgramId: string,
    decimals: number = 6,
  ): Promise<BalanceResult | null> {
    loading.value = true;
    error.value = null;

    try {
      const connection = new Connection(rpcUrl, "confirmed");
      const walletPubkey = new PublicKey(walletAddress);
      const mintPubkey = new PublicKey(mintAddress);
      const tokenProgramPubkey = new PublicKey(tokenProgramId);

      // Get the associated token account address
      const ata = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgramPubkey,
        mintPubkey,
        walletPubkey,
      );

      // Fetch the token account balance
      const accountInfo = await connection.getTokenAccountBalance(ata);
      const balanceStr = accountInfo.value.amount;

      return {
        raw: balanceStr,
        formatted: formatBalance(balanceStr, accountInfo.value.decimals),
        decimals: accountInfo.value.decimals,
      };
    } catch (err) {
      // Token account might not exist yet (balance = 0)
      if (
        err instanceof Error &&
        err.message.includes("could not find account")
      ) {
        return {
          raw: "0",
          formatted: "0.00",
          decimals,
        };
      }
      error.value =
        err instanceof Error ? err.message : "Failed to fetch Solana balance";
      console.error("Solana balance fetch error:", err);
      return null;
    } finally {
      loading.value = false;
    }
  }

  // Unified balance fetch based on chain type
  async function fetchBalance(
    chainId: string,
    rpcUrl: string,
    walletAddress: string,
    tokenAddress: string,
    decimals: number = 6,
    tokenProgramId?: string,
  ): Promise<BalanceResult | null> {
    if (chainId === "solana") {
      return fetchSolanaBalance(
        rpcUrl,
        walletAddress,
        tokenAddress,
        tokenProgramId!,
        decimals,
      );
    } else {
      return fetchEvmBalance(rpcUrl, walletAddress, tokenAddress, decimals);
    }
  }

  // Format raw balance string to human-readable format
  function formatBalance(raw: string, decimals: number): string {
    if (!raw || raw === "0") return "0.00";

    const paddedRaw = raw.padStart(decimals + 1, "0");
    const integerPart = paddedRaw.slice(0, -decimals) || "0";
    const decimalPart = paddedRaw.slice(-decimals);

    // Always show exactly 2 decimal places
    const displayDecimal = decimalPart.slice(0, 2).padEnd(2, "0");

    // Add thousand separators to integer part
    const formattedInteger = parseInt(integerPart).toLocaleString();

    return `${formattedInteger}.${displayDecimal}`;
  }

  return {
    loading,
    error,
    fetchBalance,
    fetchEvmBalance,
    fetchSolanaBalance,
  };
}
