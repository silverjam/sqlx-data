# sqlx-data-integration

Integration utilities and helpers for [sqlx-data](https://crates.io/crates/sqlx-data). This crate provides connection pooling, executor abstractions, and SQLx integration layer.

## Features

- **Connection Pooling** - Pool management and configuration
- **Executor Abstractions** - Unified interface for different executors
- **SQLx Integration** - Seamless integration with SQLx ecosystem
- **Type Definitions** - Common types and traits

## Usage

This crate is typically used through the main `sqlx-data` crate and provides foundational integration with SQLx:

```rust
use sqlx_data::{Pool, Result};

// Pool and executor types from sqlx-data-integration
let pool: Pool = Pool::connect("sqlite::memory:").await?;
```

## SQLx Compatibility

Compatible with SQLx 0.8+ and provides additional abstractions for:

- Transaction handling
- Connection management
- Executor trait implementations

For complete documentation, see the [sqlx-data documentation](https://docs.rs/sqlx-data).