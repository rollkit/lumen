# Lumen - Rollkit Integration for Reth

Lumen is a specialized integration layer that enables [Reth](https://github.com/paradigmxyz/reth) to work seamlessly with [Rollkit](https://rollkit.dev/), providing a custom payload builder that supports transaction submission via the Engine API.

## Overview

This project provides a modified version of Reth that includes:

- **Custom Payload Builder**: A specialized payload builder that accepts transactions through Engine API payload attributes
- **Rollkit-Compatible Engine API**: Modified Engine API validation to work with Rollkit's block production model
- **Transaction Support**: Full support for including transactions in blocks via the Engine API `engine_forkchoiceUpdatedV3` method

## Key Features

### 1. Engine API Transaction Support
Unlike standard Reth, Lumen accepts transactions directly through the Engine API payload attributes. This allows Rollkit to submit transactions when requesting new payload creation.

### 2. Custom Payload Builder
The `RollkitPayloadBuilder` handles:
- Transaction decoding from Engine API attributes
- Block construction with proper gas limits
- State execution and validation

### 3. Flexible Block Validation
Modified Engine API validator that:
- Bypasses block hash validation for Rollkit blocks
- Supports custom gas limits per payload
- Maintains compatibility with standard Ethereum validation where possible

## Installation

### Prerequisites

- Rust 1.82 or higher
- Git

### Building from Source

```bash
# Clone the repository
git clone https://github.com/rollkit/lumen.git
cd lumen

# Build the project
make build

# Run tests
make test
```

## Usage

### Running the Lumen Node

Basic usage:
```bash
./target/release/lumen node
```

With custom configuration:
```bash
./target/release/lumen node \
    --chain <CHAIN_SPEC> \
    --datadir <DATA_DIR> \
    --http \
    --http.api all \
    --ws \
    --ws.api all
```

### Engine API Integration

When using the Engine API, you can include transactions in the payload attributes:

```json
{
  "method": "engine_forkchoiceUpdatedV3",
  "params": [
    {
      "headBlockHash": "0x...",
      "safeBlockHash": "0x...",
      "finalizedBlockHash": "0x..."
    },
    {
      "timestamp": "0x...",
      "prevRandao": "0x...",
      "suggestedFeeRecipient": "0x...",
      "withdrawals": [],
      "parentBeaconBlockRoot": "0x...",
      "transactions": ["0x...", "0x..."],  // RLP-encoded transactions
      "gasLimit": "0x1c9c380"  // Optional custom gas limit
    }
  ]
}
```

## Architecture

### Modular Design

Lumen follows a modular architecture similar to Odyssey, with clear separation of concerns:

- **`bin/lumen`**: The main executable binary
- **`crates/common`**: Shared utilities and constants used across all crates
- **`crates/node`**: Core node implementation including the payload builder
- **`crates/rollkit`**: Rollkit-specific types and integration logic
- **`crates/e2e-tests`**: Comprehensive end-to-end tests

This modular design allows for:
- Better code organization and maintainability
- Easier testing of individual components
- Clear separation between Rollkit-specific and general node logic
- Reusable components for other projects

### Components

1. **RollkitPayloadBuilder** (`crates/node/src/builder.rs`)
   - Handles payload construction with transactions from Engine API
   - Manages state execution and block assembly

2. **RollkitEngineTypes** (`bin/lumen/src/main.rs`)
   - Custom Engine API types supporting transaction attributes
   - Payload validation and attribute processing

3. **RollkitEngineValidator** (`bin/lumen/src/main.rs`)
   - Modified validator for Rollkit-specific requirements
   - Bypasses certain validations while maintaining security

4. **Rollkit Types** (`crates/rollkit/src/types.rs`)
   - Rollkit-specific payload attributes and types
   - Transaction encoding/decoding utilities

### Transaction Flow

1. Rollkit submits transactions via Engine API payload attributes
2. `RollkitEnginePayloadAttributes` decodes and validates transactions
3. `RollkitPayloadBuilder` executes transactions and builds block
4. Block is returned via standard Engine API response

## Configuration

### Payload Builder Configuration

The payload builder can be configured with:
- `max_transactions`: Maximum transactions per block (default: 1000)
- `min_gas_price`: Minimum gas price requirement (default: 1 Gwei)

### Node Configuration

All standard Reth configuration options are supported. Key options for Rollkit integration:

- `--http`: Enable HTTP-RPC server
- `--ws`: Enable WebSocket-RPC server  
- `--authrpc.port`: Engine API port (default: 8551)
- `--authrpc.jwtsecret`: Path to JWT secret for Engine API authentication

## Development

### Project Structure

```
lumen/
├── bin/
│   └── lumen/                  # Main binary
│       ├── Cargo.toml
│       └── src/
│           └── main.rs         # Binary with Engine API integration
├── crates/
│   ├── common/                 # Shared utilities and constants
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── constants.rs
│   ├── node/                   # Core node implementation
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── builder.rs     # Payload builder implementation
│   │       └── config.rs      # Configuration types
│   ├── rollkit/                # Rollkit-specific types
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── types.rs       # Rollkit payload attributes
│   └── e2e-tests/              # End-to-end tests
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           └── *.rs            # Test files
├── etc/                        # Configuration files
│   └── lumen-genesis.json      # Genesis configuration
├── Cargo.toml                  # Workspace configuration
├── Makefile                    # Build automation
└── README.md                   # This file
```

### Running Tests

```bash
# Run all tests
make test

# Run with verbose output
make test-verbose

# Run specific test
cargo test test_name
```

### Building for Development

```bash
# Debug build
make build-dev

# Run with debug logs
make run-dev
```

## Troubleshooting

### Common Issues

1. **Transaction Decoding Errors**
   - Ensure transactions are properly RLP-encoded
   - Check that transaction format matches network requirements

2. **Block Production Failures**
   - Verify gas limits are reasonable
   - Check state availability for parent block

3. **Engine API Connection Issues**
   - Ensure JWT secret is properly configured
   - Verify Engine API port is accessible

### Debug Logging

Enable detailed logging:
```bash
RUST_LOG=debug,lumen=trace ./target/release/lumen node
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Submit a pull request

## License

This project is dual-licensed under:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

## Acknowledgments

This project builds upon the excellent work of:
- [Reth](https://github.com/paradigmxyz/reth) - The Rust Ethereum client
- [Rollkit](https://rollkit.dev/) - The modular rollup framework