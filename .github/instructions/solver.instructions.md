---
applyTo: "solver/**/*.rs"
---

# Solver Package - AI Context

## General Instructions

- Make code concise withouth sacrificing readability, safety, and maintainability.
- Ensure async code is efficient, non-blocking, and thread-safe.
- Follow Rust best practices and idiomatic patterns.
- Add comments when code is not clear or complex but not for obvious code.
- Ensure code compiles without warnings.
- Handle errors properly using ?, match, or if let.
- Use logging (slog) for important actions, errors, and state changes.
- Prefer borrowing (&T) over cloning unless ownership transfer is necessary.
- Avoid deeply nested logic—refactor with functions or combinators.
- Don't overuse clone(), use borrowing instead of cloning unless ownership transfer is needed.
- Follow the Rust Style Guide and use rustfmt for automatic formatting

## System Architecture

**Pattern**: Event-driven component system with centralized EventBus
**Flow**: `Blockchain → Listener → EventBus → Components → Events → EventBus`

### Components (`solver/src/components/`)

| Component        | Purpose                                                    |
| ---------------- | ---------------------------------------------------------- |
| EvmEventListener | Listen to EVM chain events (orders, fills, cancels)        |
| SvmEventListener | Listen to Solana program events                            |
| OrderProcessor   | Validate orders, check asset support, lifecycle management |
| InventoryManager | Track liquidity, hold assets, manage rebalancing           |
| EvmWriter        | Execute EVM transactions (fills, swaps)                    |
| EventLogger      | Log all events for observability                           |
| OrderTimer       | Track deadlines, trigger refunds                           |

**All components implement `EventHandler` trait**: `initialize()`, `handle_event()`, `name()`

### Event System (`solver/src/events/`)

- EventBus: tokio broadcast channels (capacity 1000)
- SolverEvent: Enum of all events
- Components return `Result<Vec<SolverEvent>>` from `handle_event()`

### Stores (`solver/src/stores/`)

- OrderStore: In-memory active orders with state tracking
- AssetStore: Cached supported assets from liquidity API

## Coding Rules

### Error Handling

- Use `thiserror` for errors in `error.rs`
- Component methods: `Result<Vec<SolverEvent>>`
- Always log errors before returning
- Return `SolverError::Component()`, `SolverError::Store()`, `SolverError::OrderNotFound()`

### Logging (slog)

```rust
// Create child logger per component
let logger = params.logger.new(slog::o!("component" => "ComponentName"));

// Log format
info!(logger, "Action description"; "key" => value, "order_id" => &id);
```

- `info!` for important operations
- `debug!` for verbose details
- Always include context fields

### Configuration

- File: `solver/config.yaml` (gitignored, see `config_example.yaml`)
- Structure: Environment (dev/prod/local), Network (localnet/devnet/mainnet), Chains (array), Signers, RateLimits
- Access: `params.config.field_name`

## Implementation Patterns

### New Component Checklist

1. Create `solver/src/components/my_component.rs`
2. Implement `EventHandler` trait
3. Add event types to `solver/src/events/events.rs`
4. Export from `solver/src/components/mod.rs`
5. Register in `solver/src/lib.rs`: `register_component(&component, &event_bus, &shutdown_tx, &logger)`
6. Initialize stores/connections in `initialize()`
7. Handle events in `handle_event()` - match on `SolverEvent` variants

### Event Handling Template

```rust
async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
    match event {
        SolverEvent::MyTriggerEvent(e) => {
            // Validate
            // Process
            // Return new events
            Ok(vec![SolverEvent::MyResultEvent(MyResultEvent::new(...))])
        }
        _ => Ok(vec![]), // Ignore irrelevant events
    }
}
```

### Shared State Access

```rust
// Read
let store = self.store.read().await;
let data = store.get();

// Write
let mut store = self.store.write().await;
store.update();
```

## Blockchain Integration

### EVM (alloy crate)

```rust
// Contract binding
alloy::sol! { /* ABI here */ }

// Provider
let provider = params.provider_manager.get_evm_provider(chain_id)?;

// Transaction
let tx = contract.method(args).send().await?;
```

### SVM (anchor-client)

```rust
// Use types from order_book program
use order_book::{OrderData, /* other types */};

// Client from provider_manager
let client = params.provider_manager.get_svm_client()?;
```

## Testing

### Structure

- Integration: `solver/tests/integration_tests.rs`

## Common Modifications

### New Event Type

1. Add variant to `SolverEvent` enum in `events/events.rs`
2. Add event struct with `new()` method
3. Update relevant component's `handle_event()`
4. Optionally add to `order_id()` helper if event contains order_id

## Key Files

- `solver/src/lib.rs` - Entry point, component registration
- `solver/src/config.rs` - Config loading and types
- `solver/src/events/events.rs` - All event definitions
- `solver/src/components/mod.rs` - Component trait and params
- `solver/src/error.rs` - Error types
