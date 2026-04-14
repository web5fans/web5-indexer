# Agent Guidelines for web5-indexer

This document captures the architecture, coding style, naming conventions, and recurring patterns of the `web5-indexer` project so that coding agents can work effectively within its conventions.

---

## 1. Project Architecture

- **Type:** Single-crate binary (not a Cargo workspace).
- **Edition:** Rust 2024.
- **High-level purpose:** CKB blockchain indexer that scans on-chain cells for DID / Vote / DAO data and exposes the indexed state through an Actix-web HTTP API.

### Directory layout

```
src/
├── main.rs          # Entry point: starts the HTTP server + background tokio indexing task
├── ckb.rs           # Core CKB chain-scanning logic (CkbCtx, rolling loop, cell handlers)
├── router.rs        # Actix-web route handlers and API docs (utoipa)
├── error.rs         # Central AppError enum
├── config.rs        # Env-based configuration (AppConfig)
├── crawl.rs         # HTTP client for relay / PDS crawling
├── util.rs          # Utility helpers (parsing, validation, math)
├── types.rs         # Shared domain types (Web5DocumentData, Service)
├── models.rs        # Diesel model structs (Queryable / Insertable)
├── schema.rs        # Diesel table definitions (auto-generated, do not hand-edit)
├── db/              # Database access layer (synchronous Diesel)
│   ├── mod.rs
│   ├── did.rs
│   ├── vote.rs
│   ├── dao.rs
│   └── pds.rs
└── molecules/       # Auto-generated Molecule serialization code
    ├── mod.rs
    ├── did_cell.rs
    └── vote.rs
```

### Runtime architecture

- **Dual runtime:**
  1. `main` runs an Actix-web HTTP server.
  2. A `tokio::spawn` background task continuously calls `CkbCtx::rolling()` to index CKB blocks.
- **Shared state:** An `r2d2::Pool<ConnectionManager<PgConnection>>` is shared between the web layer (via `web::Data`) and the background task.
- **Graceful shutdown:** Uses `tokio_util::sync::CancellationToken` + `tokio::select!` to stop the rolling task on `ctrl-c` or cancel signals.
- **DB layer is sync:** All Diesel operations are synchronous. The router sometimes offloads them with `actix_web::web::block` (used sparingly).

---

## 2. Coding Style

### Formatting

- Standard `rustfmt` formatting (4-space indentation, same-line braces).
- Group imports as:
  1. `crate::` items (often grouped in a single `use crate::{...}` block)
  2. External crates
  3. `std::` items

### Error handling

- **Centralized error type:** `AppError` in `error.rs` uses `derive_more::Display`.
- **Map-everything pattern:** Fallible operations are almost always chained with `.map_err(|e| AppError::Variant(e.to_string()))?`.
- **HTTP mapping:** `AppError` implements `actix_web::ResponseError`, returning 404 or 500.
- **Special swallowing:** `handle_error(e)` in `error.rs` intentionally ignores `DbRecordNotFound` and `DbInsOrUpFailed` so the indexer can keep rolling.
- **Unwrap usage:** `unwrap()` appears ~35 times, mostly in startup / config paths and a few indexing hot paths. Prefer `?` + `AppError` in new code.

### Async patterns

- `#[actix_web::main]` on `async fn main`.
- Background work spawns via `tokio::spawn`.
- `tokio::select!` is used for cancellation and timeout loops.
- CKB RPC and HTTP crawl methods are `async`; DB methods remain synchronous.

### Database access style

- Import Diesel DSL aliases explicitly per table:
  ```rust
  use crate::schema::indexer::did_record::dsl as DidRecordSchema;
  ```
- Chain `.optional()`, then `.ok_or(AppError::...)?` or `.map_err(...)?`.
- Inserts frequently use `.on_conflict_do_nothing()` for idempotent re-indexing.

---

## 3. Naming Conventions

| Category        | Convention | Examples |
|-----------------|------------|----------|
| Files / modules | `snake_case` | `ckb.rs`, `did_cell.rs`, `db/mod.rs` |
| Structs / Enums | `PascalCase` | `AppConfig`, `CkbCtx`, `RollingResult`, `AppError` |
| Functions       | `snake_case` | `query_valid_did_doc`, `did_output_handle` |
| Variables       | `snake_case` | `ckb_addr`, `query_height`, `out_inx` |
| Type aliases    | `PascalCase` | `DbPool` |
| DB columns      | `camelCase` in SQL, mapped via `#[diesel(column_name = "...")]` | `ckbAddress`, `txHash`, `signingKey` |

### Notable quirks

- Some abbreviated boolean parameter names exist in the codebase (e.g., `pon` = positive-or-negative, `qon` = query-or-not). Prefer descriptive names in new code.
- Minor inconsistencies exist (`time_stamp` vs `timestamp`, `out_inx` vs `out_index`). Follow the majority `snake_case` pattern.

---

## 4. Writing Habits

### Comments & documentation

- **Inline comments are sparse.** Only a handful of `//` comments appear outside generated code.
- **Doc comments (`///`) are used in `router.rs`** for OpenAPI generation via `utoipa`.
- `models.rs` and `schema.rs` carry generated headers; do not remove them.

### Testing

- **There are currently no tests.** If you add tests, create a `tests/` directory or `#[cfg(test)]` modules and follow standard Rust testing conventions.

### Logging

- Uses the `tracing` crate (`#[macro_use] extern crate tracing;` in `main.rs`).
- **Structured prefixes** are common: `[did]:`, `[vote]:`, `[dao]:`, `[pds]:`, `[relay]:`, `[crawl]:`.
- **Every DB function** (24 total) is annotated with `#[tracing::instrument(skip_all)]` for automatic span generation. Preserve this pattern when adding new DB functions.
- Levels: `info!` for milestones, `error!` for failures, `warn!` for non-fatal issues, `trace!` for low-level tracing.

---

## 5. Build System & Tooling

### Cargo

- Single package, `edition = "2024"`, `version = "0.1.18"`, `license = "MIT"`.
- No custom `[features]` block.
- A local `config.toml` configures a USTC cargo registry mirror.

### Key dependencies

- **Web:** `actix-web`, `actix-cors`, `actix-files`
- **Async:** `tokio` (full), `tokio-util`
- **DB:** `diesel` 2.x with `chrono`, `postgres`, `r2d2`
- **CKB:** `ckb-sdk`, `ckb-types`, `ckb-jsonrpc-types`, `molecule`
- **Serialization:** `serde`, `serde_json`, `serde_ipld_dagcbor`
- **API docs:** `utoipa`, `utoipa-swagger-ui`
- **Logging:** `tracing`, `tracing-subscriber`

### Tooling

- **Diesel CLI** for migrations (`diesel.toml` points to schema `indexer`).
- **Docker:** 2-stage `Dockerfile` (builder + runtime, both `rust:latest`).
- **VS Code:** `.vscode/launch.json` with LLDB cargo debugging.
- **CI/CD:** None currently configured.

---

## 6. Recurring Patterns

### Configuration

- `AppConfig::from_env()` reads environment variables with `.unwrap_or(default)` fallbacks.
- `dotenvy::dotenv().ok();` is called early in `main` to optionally load `.env`.
- A small `env_int()` helper parses numeric env vars.

### CKB indexing loop

- `CkbCtx::rolling()` fetches one block at a time by height.
- Iterates transactions, then inputs (for spends / deletions) and outputs (for creations).
- Maintains an in-memory `HashSet<(H256, i32, CellType)>` of valid cells to know which inputs correspond to tracked cells.
- `AppMode` (`DID | VOTE | DAO`) controls which cell types are processed.

### Code generation

- `src/molecules/did_cell.rs` and `src/molecules/vote.rs` are fully auto-generated from `.mol` schemas.
- Do not hand-edit generated files; update the source `.mol` files and regenerate instead.

### HTTP API design

- REST-ish endpoints with path and query parameters.
- Swagger UI is served at `/swagger-ui/`.
- CORS is enabled globally with `allow_any_origin()`.

---

## 7. Guidelines for Agents

- **Keep changes minimal.** The codebase is pragmatic and flat; avoid deep module hierarchies.
- **Preserve the sync-DB + async-web pattern.** If a new DB function is added, keep it synchronous and annotate it with `#[tracing::instrument(skip_all)]`.
- **Use `AppError` for all error propagation.** Map foreign errors into `AppError` variants rather than introducing new error types.
- **Respect generated files.** Do not manually edit `schema.rs` or anything under `src/molecules/`.
- **Follow existing import grouping.** Use `crate::{...}` grouped blocks where possible.
- **Add `///` doc comments** on new router handlers so `utoipa` can pick them up for Swagger.
- **Prefer `snake_case`** for files, modules, functions, and variables; `PascalCase` for types.
