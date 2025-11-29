# sqlx-data-params

Data parameter utilities for [sqlx-data](https://crates.io/crates/sqlx-data). This crate provides advanced pagination, dynamic filtering, sorting, and type-safe query parameters for database operations.

## Features

### Pagination Strategies

- **Serial Pagination** - Traditional page-based pagination
- **Slice Pagination** - Offset/limit with total count
- **Cursor Pagination** - High-performance infinite scroll

### Dynamic Queries

- **FilterBuilder** - Type-safe WHERE clause construction
- **SortBuilder** - ORDER BY clause building
- **SearchBuilder** - Full-text search capabilities

## Usage

```rust
use sqlx_data::{ParamsBuilder, FilterValue};

let params = ParamsBuilder::new()
    .serial()
        .page(1, 20)
        .done()
    .filter()
        .gt("age", 18)
        .like("name", "%Alice%")
        .done()
    .sort()
        .desc("created_at")
        .done()
    .build();
```

For complete documentation, see the [sqlx-data documentation](https://docs.rs/sqlx-data).