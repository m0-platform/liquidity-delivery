# Solver

The solver tracks and processes orders from the M0 liquidity delivery protocol.

## Configuration

The solver uses environment variables for configuration. You can set these directly or create a `.env` file in the solver directory.

### Environment Variables

- **ENV**: Determines the execution environment
  - Values: `development` (or `dev`), `production` (or `prod`)
  - Default: `development`
  - Effect: In production mode, logs are output in JSON format for structured logging

- **NETWORK**: Specifies which Solana network to connect to
  - Values: `local` (or `localhost`), `devnet` (or `dev`), `mainnet` (or `main`)
  - Default: `local`

### Example Configuration

Create a `.env` file:
```bash
ENV=development
NETWORK=devnet
```

Or set environment variables directly:
```bash
export ENV=production
export NETWORK=mainnet
./solver
```

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

```
