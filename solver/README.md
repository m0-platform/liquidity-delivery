# Solver

The solver tracks and processes orders from the M0 liquidity delivery protocol.

## Architecture Overview

The solver application uses an event-driven architecture comprised of multiple components that communicate through an event bus. The design emphasizes:

- **Decoupling**: Components only interact through events
- **Asynchronous Processing**: All components run concurrently
- **State Management**: Centralized stores update before components process events
- **Simplicity**: Clear separation of concerns with focused components

### Core Components

- Events (`events.rs`)
- Stores (`stores.rs`)
- Components (`components/`)
