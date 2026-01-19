import { getOrdersUrl, NetworkType } from "@/config/network";
import { computed, MaybeRef, ref, toValue } from "vue";

export interface TransactionRecord {
  transaction_hash: string;
  event: string;
}

export interface TrackedOrder {
  order_id: string;
  status: string;
  version: number;
  nonce: number;
  origin_chain_id: number;
  dest_chain_id: number;
  fill_deadline: number;
  sender: string;
  recipient: string;
  token_in: string;
  token_out: string;
  amount_in: string;
  amount_out: string;
  filled_amount: string;
  solver: string;
  transaction_history: TransactionRecord[];
}

export interface OrdersResponse {
  orders: TrackedOrder[];
  count: number;
}

export function useOrders(networkRef: MaybeRef<NetworkType>) {
  const orders = ref<TrackedOrder[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const solverUrl = computed(() => getOrdersUrl(toValue(networkRef)));

  async function fetchOrders(): Promise<TrackedOrder[]> {
    loading.value = true;
    error.value = null;

    try {
      const response = await fetch(`${solverUrl.value}/orders`);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data: OrdersResponse = await response.json();
      orders.value = data.orders;
      return data.orders;
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : "Failed to fetch orders";
      return [];
    } finally {
      loading.value = false;
    }
  }

  function getOrder(orderId: string): TrackedOrder | undefined {
    let id = orderId.startsWith("0x") ? orderId.slice(2) : orderId;
    return orders.value.find((order) => order.order_id === id);
  }

  function getOrdersBySender(sender: string): TrackedOrder[] {
    return orders.value.filter(
      (order) => order.sender.toLowerCase() === sender.toLowerCase(),
    );
  }

  return {
    orders,
    loading,
    error,
    fetchOrders,
    getOrder,
    getOrdersBySender,
  };
}
