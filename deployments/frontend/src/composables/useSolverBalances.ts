import { getOrdersUrl, NetworkType } from "@/config/network";
import { computed, MaybeRef, ref, toValue } from "vue";

export interface SolverBalance {
  chain: string;
  address: string;
  symbol: string;
  decimals: number;
  balance: string;
}

export interface SolverBalancesResponse {
  balances: SolverBalance[];
  count: number;
}

export function useSolverBalances(networkRef: MaybeRef<NetworkType>) {
  const balances = ref<SolverBalance[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const solverUrl = computed(() => getOrdersUrl(toValue(networkRef)));

  async function fetchBalances(): Promise<SolverBalance[]> {
    loading.value = true;
    error.value = null;

    try {
      const response = await fetch(`${solverUrl.value}/balances`);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data: SolverBalancesResponse = await response.json();
      balances.value = data.balances;
      return data.balances;
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : "Failed to fetch balances";
      return [];
    } finally {
      loading.value = false;
    }
  }

  // Group balances by chain
  const balancesByChain = computed(() => {
    const grouped = new Map<string, SolverBalance[]>();
    for (const balance of balances.value) {
      const existing = grouped.get(balance.chain) || [];
      existing.push(balance);
      grouped.set(balance.chain, existing);
    }
    return grouped;
  });

  // Calculate total USD value (placeholder - would need price feeds)
  const totalCount = computed(() => balances.value.length);

  return {
    balances,
    balancesByChain,
    totalCount,
    loading,
    error,
    fetchBalances,
  };
}
