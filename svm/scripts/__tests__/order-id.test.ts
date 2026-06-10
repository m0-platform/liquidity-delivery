import { computeOrderId, encodeOrderData, OrderData } from "../lib/order-id";

describe("Order ID computation", () => {
  // Test vector from svm/programs/order_book/src/state/orders.rs:148-193
  const testOrder: OrderData = {
    version: 1,
    sender: new Uint8Array(32).fill(1),
    nonce: 42n,
    originChainId: 1,
    destChainId: 2,
    createdAt: 12345500000n,
    fillDeadline: 1234567890n,
    tokenIn: new Uint8Array(32).fill(5),
    tokenOut: new Uint8Array(32).fill(2),
    amountIn: 1000n,
    amountOut: 2000n,
    recipient: new Uint8Array(32).fill(3),
    solver: new Uint8Array(32).fill(4),
  };

  it("should encode to exactly 226 bytes", () => {
    const encoded = encodeOrderData(testOrder);
    expect(encoded.length).toBe(226);
  });

  it("should produce consistent order IDs", () => {
    const id1 = computeOrderId(testOrder);
    const id2 = computeOrderId(testOrder);
    expect(Buffer.from(id1).toString("hex")).toBe(Buffer.from(id2).toString("hex"));
  });

  it("should match the EVM abi.encodePacked output from cast", () => {
    const encoded = encodeOrderData(testOrder);

    // Generated with:
    //   cast abi-encode --packed \
    //     'f(uint16,bytes32,uint64,uint32,uint32,uint64,uint64,bytes32,bytes32,uint128,uint128,bytes32,bytes32)' \
    //     1 0x0101..01 42 1 2 12345500000 1234567890 0x0505..05 0x0202..02 1000 2000 0x0303..03 0x0404..04
    const expectedEncoding =
      "0001" +
      "0101010101010101010101010101010101010101010101010101010101010101" +
      "000000000000002a" +
      "00000001" +
      "00000002" +
      "00000002dfd96160" +
      "00000000499602d2" +
      "0505050505050505050505050505050505050505050505050505050505050505" +
      "0202020202020202020202020202020202020202020202020202020202020202" +
      "000000000000000000000000000003e8" +
      "000000000000000000000000000007d0" +
      "0303030303030303030303030303030303030303030303030303030303030303" +
      "0404040404040404040404040404040404040404040404040404040404040404";

    expect(encoded.toString("hex")).toBe(expectedEncoding);
  });

  it("should match the EVM keccak256 hash from cast keccak", () => {
    // Verified with: cast keccak "$(cast abi-encode --packed ...)" using the test vector above
    const orderId = computeOrderId(testOrder);
    const orderIdHex = Buffer.from(orderId).toString("hex");

    expect(orderId.length).toBe(32);
    expect(orderIdHex).toBe(
      "e92c60ff7210a8b0b60931e03ba5a75960898baa06c34a8e6828d93c2cc559d3"
    );
  });
});
