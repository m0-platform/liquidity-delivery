import { ref } from 'vue'

export interface TrackedOrder {
  order_id: string
  origin_chain_id: number
  sender: string
  token_in: string
  amount_in: string
  dest_chain_id: number
  token_out: string
  amount_out: string
  solver: string
}

export interface OrderDetails {
  order_id: string
  status: string
  version: number
  sender: string
  nonce: number
  dest_chain_id: number
  fill_deadline: number
  cancel_requested_at: number
  token_in: string
  token_out: string
  amount_in: string
  amount_out: string
  recipient: string
  solver: string
}

export interface OrdersResponse {
  orders: TrackedOrder[]
  count: number
}

export interface OrderDetailResponse {
  order: OrderDetails | null
  error: string | null
}

export function useOrders() {
  const orders = ref<TrackedOrder[]>([])
  const selectedOrder = ref<OrderDetails | null>(null)
  const loading = ref(false)
  const detailLoading = ref(false)
  const error = ref<string | null>(null)
  const detailError = ref<string | null>(null)

  const quoterUrl = import.meta.env.VITE_QUOTER_URL || 'http://localhost:3000'

  async function fetchOrders(sender?: string): Promise<TrackedOrder[]> {
    loading.value = true
    error.value = null

    try {
      const url = sender
        ? `${quoterUrl}/orders?sender=${encodeURIComponent(sender)}`
        : `${quoterUrl}/orders`

      const response = await fetch(url)

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`)
      }

      const data: OrdersResponse = await response.json()
      orders.value = data.orders
      return data.orders
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to fetch orders'
      return []
    } finally {
      loading.value = false
    }
  }

  async function fetchOrderDetails(orderId: string): Promise<OrderDetails | null> {
    detailLoading.value = true
    detailError.value = null
    selectedOrder.value = null

    try {
      const response = await fetch(`${quoterUrl}/orders/${orderId}`)

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`)
      }

      const data: OrderDetailResponse = await response.json()

      if (data.error) {
        throw new Error(data.error)
      }

      selectedOrder.value = data.order
      return data.order
    } catch (err) {
      detailError.value = err instanceof Error ? err.message : 'Failed to fetch order details'
      return null
    } finally {
      detailLoading.value = false
    }
  }

  function clearSelectedOrder() {
    selectedOrder.value = null
    detailError.value = null
  }

  return {
    orders,
    selectedOrder,
    loading,
    detailLoading,
    error,
    detailError,
    fetchOrders,
    fetchOrderDetails,
    clearSelectedOrder,
  }
}
