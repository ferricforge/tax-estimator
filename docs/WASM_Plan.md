# WASM Enablement Plan

## Goal

Enable a browser-hosted (WASM) version of the tax estimator while preserving the existing native CLI + SQLite workflow.

This plan is based on the current workspace layout and code boundaries in:

- `tax-core` (domain models, calculations, repository traits)
- `tax-db-sqlite` (SQLx/SQLite backend + migrations/seeds)
- `tax-ui` (native CLI entrypoint + data loading/formatting)
- `tax-data` (CSV tax bracket loader CLI)

## Technology Choices (Explicit)

### UI framework (Rust/WASM)

Chosen stack: `Yew` (CSR mode) inside `tax-ui` for the browser presentation path.

Why:

- Mature, widely used Rust/WASM UI framework with active ecosystem and docs.
- Pure Rust component model (fits the "stay in Rust as much as possible" requirement).
- Straightforward integration with `wasm-bindgen` / `web-sys` / browser APIs.
- Works well with `Trunk` for a Rust-first app workflow while keeping application logic in Rust.

Scope note:

- `tax-ui` remains the presentation crate.
- Native CLI and browser UI become two target-specific faces of the same crate.

### WASM toolkit

Chosen build/tooling baseline:

- Rust target: `wasm32-unknown-unknown`
- App build/dev tool: `Trunk`
- `wasm-bindgen` interop in the browser build pipeline
- `wasm-pack` as an optional secondary tool (library packaging / experimentation)

Trunk-first rationale:

- Better fit for a Rust-first application (build, watch, serve, asset pipeline in one tool).
- Simpler local developer workflow than `wasm-pack + Vite + cargo watch` for a Yew app.
- Reduces the amount of JS shell/bootstrap code needed.

Why this matches the project:

- Keeps the Rust crate as the primary source of UI/app logic.
- Keeps the browser app path centered on `tax-ui` instead of a JS bundler project.
- Still leaves room to use `wasm-pack` later if an npm-package style distribution is needed.

### Local development shell

Chosen local dev workflow:

- `Trunk` for Rust→WASM builds, watch mode, local dev server, and static asset bundling
- `cargo watch` only where helpful for non-Trunk tasks (e.g. data generator pipelines)

Practical shape:

- Add Trunk app files to `tax-ui` (e.g. `index.html`, optional `Trunk.toml`, CSS/assets, static data).
- Keep static browser assets in a Trunk-managed directory (e.g. `tax-ui/static/`).
- Run one main process in dev:
  - `trunk serve` -> builds, watches, and serves the WASM app with assets

## Current Workspace Evaluation (WASM Readiness)

### `tax-core` (mostly portable, good starting point)

What already helps:

- Calculation logic is pure Rust and independent of filesystem/network/database.
- Models are `serde`-serializable and suitable for JS/WASM interop.
- Database access is abstracted behind `TaxRepository` and `RepositoryFactory`.

Likely WASM friction points:

- `TaxRepository` and `RepositoryFactory` are `Send + Sync` traits and use `async_trait`.
- Browser backends often use `Rc<RefCell<...>>` / JS handles and `!Send` futures.
- `chrono`/`tracing` are usually fine, but feature flags should be reviewed for wasm target size/compatibility.

Conclusion:

- `tax-core` is the right crate to reuse directly in WASM.
- Small trait-bound adjustments may be needed to support a browser repository implementation cleanly.

### `tax-db-sqlite` (native-only today)

Major blockers for direct browser use:

- Depends on `sqlx` with `runtime-tokio` + SQLite.
- Uses filesystem APIs (`std::fs::read_dir`, `std::fs::read_to_string`) to load seed SQL.
- Uses `sqlx::migrate!` migrations from on-disk SQL files.
- Assumes file or in-memory SQLite via native runtime.

Conclusion:

- Do not try to reuse `tax-db-sqlite` directly in wasm.
- Treat it as the native backend and keep it intact.

### `tax-ui` (incomplete, and the right place to evolve the presentation layer)

Major blockers:

- `clap` CLI entrypoint (`#[tokio::main]` in `src/main.rs`).
- Hard dependency on `tax-db-sqlite`.
- `build_registry()` registers only `SqliteRepositoryFactory`.

What is reusable:

- `load_tax_year_data()` is backend-agnostic and can be reused if moved or shared.
- Formatting and orchestration logic can be split from CLI concerns.
- The crate already has a `lib.rs`, which is a good foundation for target-specific UI paths.

Conclusion:

- `tax-ui` should become the presentation-layer crate for both native and WASM.
- The current CLI code should be isolated as a native-only path (binary + native modules), while new browser UI modules are added behind target/features.

### `tax-data` (tooling crate, native-focused)

Current role:

- CSV parsing/loading for tax bracket import.
- CLI uses filesystem and sqlite backend directly.

WASM relevance:

- The parsing logic is reusable.
- The CLI path is not.

Conclusion:

- Keep `tax-data` native for maintenance/admin workflows.
- Reuse its parsing logic or generated artifacts for browser reference data.

## Database Strategy (Hardest Part)

Short answer: OPFS-backed SQLite can work, but it should be a later phase unless strict SQL parity/offline persistence is required immediately.

### Option A (Recommended First): Browser-native repository without SQLite

Design:

- Keep reference tax data as generated JSON assets (served by Trunk's static asset pipeline), not hard-coded Rust constants.
- Load reference data into memory at app startup and pass it into the browser repository.
- Store user-created estimates in IndexedDB (not `localStorage`).
- Implement `TaxRepository` in a new browser backend crate (e.g. `tax-db-browser`).

IndexedDB approach (explicit):

- Prefer a Rust-first implementation using:
  - `indexed_db_futures` for IndexedDB access
  - `serde_wasm_bindgen` for Rust <-> JS value serialization
  - `web-sys` / `js-sys` only where the wrapper crate does not cover needed APIs
- Avoid custom handwritten JS for data access; keep JS limited to minimal browser bootstrapping and any test harness glue.

Why this is best for this codebase now:

- Reference data is small and mostly static.
- Query patterns are simple and well-known (lookup by year/status, list estimates).
- Avoids wasm SQLite integration complexity, worker bridging, and migration tooling.
- Faster path to a working browser MVP.

Tradeoffs:

- No SQL migration reuse in browser.
- Need a small amount of browser-specific storage code.

### Portable ID strategy (IndexedDB <-> SQLite <-> future OPFS/SQLite)

Decision:

- Use a single portable `id` for `tax_estimate` across all backends.
- `tax_estimate.id` should be a UUIDv7 string, not a backend-local autoincrement integer.
- The same `id` value is used in SQLite rows, IndexedDB records, export files, import/upsert, and future OPFS/SQLite migration.

Why UUIDv7:

- Offline-safe generation in browser and native environments.
- Stable identity for mutable records (unlike content-derived UUIDv5).
- Time-ordered characteristics help with sorting/debugging and import conflict handling.

Why not UUIDv5 as the primary ID:

- UUIDv5 is deterministic from input, which is a poor fit for mutable user-edited estimates.
- If derived from record content, the ID changes when the record changes.
- If derived from legacy/local IDs, it preserves backend-specific identity issues.

Schema alignment impact (intentional breaking change while unpublished):

- Update SQLite `tax_estimate.id` from `INTEGER PRIMARY KEY AUTOINCREMENT` to `TEXT PRIMARY KEY`.
- Use the same UUIDv7 `id` as the IndexedDB key path for browser storage.
- Keep composite/natural keys in reference tables unchanged.

Repository/model impact (intentional breaking change):

- Change `TaxEstimate.id` and related repository method parameters away from `i64` to a portable ID type.
- Prefer a newtype in `tax-core` (e.g. `EstimateId`) over raw `String`.

Export/import and migration rule:

- Export payloads include the portable `id`.
- Import/upsert matches by `id` exactly (no ID remapping).
- Local backend migration (IndexedDB -> SQLite or SQLite -> IndexedDB/OPFS) preserves `id` unchanged.

Reference data interoperability rule:

- For estimates, interchange payloads should use `filing_status_code` (e.g. `S`, `MFJ`) rather than assuming numeric seed IDs are universal.

### Option B (Later / Optional): OPFS-backed SQLite in WASM

Design:

- Run a SQLite WASM engine in the browser.
- Persist DB file in OPFS (Origin Private File System).
- Apply migrations/seeds from embedded SQL strings.
- Expose a repository implementation compatible with `TaxRepository`.

Pros:

- Maximum parity with native schema and SQL behavior.
- Reuses migration/seed concepts.
- Better for future complex querying/reporting.

Cons (significant):

- More moving parts (WASM SQLite runtime + JS glue + persistence integration).
- Browser compatibility/testing complexity.
- Often best run in a Web Worker for responsiveness.
- Higher bundle size and operational complexity than needed for current lookup-heavy usage.

Recommendation:

- Start with Option A.
- Keep the repository abstraction so Option B can be added later behind a feature/backend name (e.g. `"browser-sqlite"`).

## Trait and `cfg` Design Rules (Explicit)

These rules are intended to avoid target-specific divergence in the domain/repository interfaces.

1. Keep repository method signatures identical across targets.

- Do not `#[cfg]` individual trait methods in `TaxRepository` / `RepositoryFactory`.
- `cfg` differences belong in implementations, module wiring, and dependency selection.

2. Put target-specific async-trait behavior behind `cfg_attr` on trait definitions and impls.

- Native path: `#[async_trait]`
- WASM path: `#[async_trait(?Send)]`

Practical pattern (for both trait definitions and impls):

```rust
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
```

3. Avoid `Send + Sync` requirements in shared UI/browser-facing state unless truly needed.

- Browser UI and IndexedDB handles are commonly single-threaded.
- Require `Send + Sync` in native-specific orchestration only where concurrency actually demands it.

4. Use target-specific modules instead of target-specific branches inside functions.

- Prefer:
  - `tax_ui::platform::native::*`
  - `tax_ui::platform::web::*`
- Avoid large `if cfg!(...)` branches in shared code.

5. Prefer Cargo target-specific dependency sections for platform crates.

- Keep `tax-db-sqlite`, `clap`, and `tokio` out of wasm dependency graphs.
- Keep browser crates (`yew`, `web-sys`, `wasm-bindgen-*`) out of native CLI builds unless needed.

## Proposed Architecture Changes

### Phase 1: Restructure `tax-ui` into a target-aware presentation crate

Step 1: Split `tax-ui` into shared UI/app logic vs native CLI shell

- Keep `tax-ui` as the presentation-layer crate.
- Move CLI-specific code (`clap`, `#[tokio::main]`, stdout formatting concerns) behind native-only modules and/or a native-only binary target.
- Preserve a target-agnostic `tax-ui` library surface for shared orchestration, state mapping, and formatting helpers that make sense in both targets.

Why this step:

- The crate is incomplete and is the natural place to build the WASM presentation layer.
- This avoids creating a second presentation crate before the first one is fully shaped.

Step 2: Extract backend-agnostic app logic from the current CLI-oriented module layout

- Move `load_tax_year_data()` and the `TaxYearData` types to a new crate (e.g. `tax-app-core`) or into `tax-core` if the team wants a broader domain/service crate.
- Alternatively (and likely better for this workspace), keep these in `tax-ui` but in a shared module that does not depend on `tax-db-sqlite` or `clap`.
- Keep CLI formatting/printing in native-only `tax-ui` modules.

Why this step:

- `tax-ui` currently hard-links presentation orchestration to the native sqlite backend.
- WASM UI will need the same orchestration logic without `clap` or native sqlite.

Step 3: Remove backend registration coupling from shared `tax-ui` code

- Replace `tax-ui::app::build_registry()` with backend-specific registry builders:
  - native CLI builder registers SQLite
  - browser/WASM builder registers browser backend

Why this step:

- Prevents accidental wasm compilation of `tax-db-sqlite`.

Step 4: Review async trait bounds for browser compatibility

- Evaluate whether `TaxRepository` / `RepositoryFactory` need target-specific `Send` requirements.
- If needed, use `#[async_trait(?Send)]` for wasm implementations (or split traits / use cfg-gated bounds).

Why this step:

- Browser storage + JS interop often produce `!Send` futures/handles.

### Phase 2: Add a browser repository backend (`tax-db-browser`)

Create a new crate, e.g. `tax-db-browser`, with a `TaxRepository` implementation.

Initial storage model (recommended):

- Reference tables:
  - load generated JSON asset(s) at runtime (served as static files via Trunk)
  - parse once and keep in-memory structs for fast lookup
- User estimates:
  - persist to IndexedDB
  - serialize with `serde`

Backend behavior mapping:

- `get_tax_year_config`, `list_filing_statuses`, `get_standard_deduction`, `get_tax_brackets`
  - serve from the in-memory reference dataset loaded from generated JSON assets
- `create_estimate`, `get_estimate`, `update_estimate`, `delete_estimate`, `list_estimates`
  - store in IndexedDB

Notes:

- Use the same UUIDv7 `id` string as the IndexedDB primary key (`keyPath`) to preserve portability with SQLite exports/imports and future OPFS migration.
- Use UTC timestamps generated in browser-compatible Rust/JS interop code.
- Prefer `indexed_db_futures` + `serde_wasm_bindgen` for IndexedDB access/serialization, with `web-sys` used only as needed.

### Phase 3: Implement the WASM presentation path inside `tax-ui`

Use `tax-ui` as the web presentation crate instead of introducing a new `tax-web` crate first.

Responsibilities in `tax-ui` (WASM path):

- WASM entrypoint (feature/target-gated)
- Yew CSR component tree and routing/state (as needed)
- Form/UI state handling
- Calls into `tax-core` calculations and shared `tax-ui` orchestration layer
- Uses `tax-db-browser` backend
- Uses `wasm_bindgen_futures::spawn_local` for UI-triggered async work (loading reference data, repository CRUD, startup hydration)

Responsibilities in `tax-ui` (native path):

- Existing CLI binary and stdout rendering
- Native backend registration (`tax-db-sqlite`)

Build shape:

- Keep `tax-ui` library target for shared logic
- Add `cdylib` support explicitly for wasm packaging
- Gate native-only dependencies (`clap`, `tokio`, `tax-db-sqlite`) so wasm builds do not pull them in

Async execution rule (browser path):

- No `tokio` runtime in the browser UI path.
- Use `wasm_bindgen_futures::spawn_local` from Yew event handlers/effects for async repository calls.
- If OPFS SQLite is added later and heavy work is required, move DB execution to a Web Worker and keep the UI thread on `spawn_local` for message passing.

Note:

- A separate web shell crate can still be added later if build tooling or deployment packaging becomes awkward, but it is not the recommended first step given the current state of `tax-ui`.

### Phase 4: Data packaging pipeline for browser reference data

Current native backend loads SQL seed files from disk at runtime. Browser builds cannot rely on that.

Decision (explicit):

- Do not hard-code reference tax data in Rust source.
- Use canonical JSON files in the repository as the source of truth for browser-consumable reference data.
- Generate SQLite seed SQL from the canonical JSON (not the other way around).

Recommended repository shape:

- `tax-data/reference/` -> canonical JSON inputs (versioned, human-reviewable)
- `tax-data/src/bin/...` -> generator commands
- `tax-db-sqlite/seeds/` -> generated SQL seeds for native sqlite backend
- `tax-ui/static/data/` (or equivalent) -> generated browser JSON assets served by Trunk

Why JSON as canonical source:

- Represents all tables cleanly (tax year config, filing statuses, deductions, brackets).
- Natural format for browser loading without code generation.
- Easier to validate in CI than SQL parsing.

Migration path from current state:

1. Preserve existing SQL seeds temporarily.
2. Add a `tax-data` generator that emits canonical JSON from current curated data sources (initially the existing CSV + manually specified config/status/deduction values).
3. Add a second generator that emits `tax-db-sqlite` seed SQL from canonical JSON.
4. Switch CI checks to verify generated SQL is up to date.
5. Make browser path load the generated JSON asset at runtime.

Validation rules for the generator (CI):

- JSON schema validation
- Unique filing status IDs/codes
- Brackets sorted and contiguous per `(tax_year, filing_status)`
- Exactly one open-ended bracket per `(tax_year, filing_status)`
- Generated SQL deterministic (stable formatting/order)

## OPFS / SQLite Track (Optional Phase 5)

If SQL parity becomes necessary after the browser MVP:

1. Add a new backend crate (e.g. `tax-db-browser-sqlite`)
2. Embed migrations/seeds as strings (no filesystem reads at runtime)
3. Initialize SQLite in a worker and persist DB file in OPFS
4. Implement `TaxRepository` over async message passing to the worker
5. Keep the simpler `tax-db-browser` backend as a fallback for unsupported browsers

Decision trigger for doing this:

- You need complex querying/reporting, strict schema parity, or cross-platform data portability beyond what IndexedDB objects provide.

## Build and CI Plan

### `tax-ui` Cargo.toml shape (exact pattern)

`tax-ui` should become a dual-target crate with an explicit `cdylib` output for wasm packaging.

Recommended shape (illustrative; versions should come from `[workspace.dependencies]`):

```toml
[package]
name = "tax-ui"
version.workspace = true
edition.workspace = true

[lib]
name = "tax_ui"
path = "src/lib.rs"
crate-type = ["rlib", "cdylib"]

[[bin]]
name = "tax-ui"
path = "src/main.rs"
required-features = ["native-cli"]

[features]
default = ["native-cli"]
native-cli = []
web-ui = []

[dependencies]
anyhow.workspace = true
rust_decimal.workspace = true
serde.workspace = true
tax-core = { path = "../tax-core" }
thiserror.workspace = true
tracing.workspace = true
uuid = { version = "1", features = ["v7", "serde"] }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
clap.workspace = true
tokio.workspace = true
tracing-subscriber.workspace = true
tax-db-sqlite = { path = "../tax-db-sqlite" }

[target.'cfg(target_arch = "wasm32")'.dependencies]
tax-db-browser = { path = "../tax-db-browser" }
yew = { version = "0.21", features = ["csr"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
serde-wasm-bindgen = "0.6"
indexed_db_futures = "0.6"
web-sys = { version = "0.3", features = [
  "Window",
  "Document",
  "HtmlElement",
  "HtmlInputElement",
  "Storage",
  "console",
] }
```

Notes:

- The browser path should not depend on `tokio`, `clap`, or `tax-db-sqlite`.
- The native CLI binary is explicitly feature-gated so wasm checks do not try to compile it.
- `uuid` is included in shared dependencies because the portable `tax_estimate.id` is generated/parsed in both native and wasm paths.
- If the project prefers stricter feature control, add `required-features = ["web-ui"]` to wasm-specific exported entry modules/build scripts (not needed for the library itself).

### Workspace changes

- Add a new browser storage backend crate (`tax-db-browser`)
- Evolve `tax-ui` into a dual-target presentation crate (native CLI + WASM UI path)
- Keep `tax-db-sqlite` and `tax-data` native-only

### Target checks to add

- `cargo check -p tax-core --target wasm32-unknown-unknown`
- `cargo check -p tax-db-browser --target wasm32-unknown-unknown`
- `cargo check -p tax-ui --target wasm32-unknown-unknown --no-default-features --features web-ui`
- Existing native checks/tests remain for `tax-db-sqlite`, `tax-ui`, `tax-data`

### Local dev commands (Trunk-first)

Recommended developer workflow for hot reload and bundling:

1. Run the WASM app with Trunk (single-process dev loop):

```bash
cd tax-ui
trunk serve --open --no-default-features --features web-ui
```

2. If reference data is generated, run a separate watcher for the generator (optional):

```bash
cargo watch -w tax-data -w tax-ui/static/data \
  -s "cargo run -p tax-data --bin <generator-bin>"
```

Notes:

- `Trunk` handles rebuilding, watching, and serving the Rust/WASM app and static assets.
- `cargo watch` is optional and only needed for auxiliary tasks (e.g. regenerating JSON reference assets).
- Keep `wasm-pack` available for ad hoc packaging experiments, but not as the default local dev loop.

### Testing strategy

1. `tax-core` unit + doc tests (existing)

- Continue using `cargo test` as the primary logic correctness gate.

2. Shared repository conformance test suite (new, required for all backends)

- Create one backend-agnostic conformance suite (single source of truth) that validates `TaxRepository` behavior.
- Run the same test cases against:
  - `tax-db-sqlite`
  - `tax-db-browser`
  - any future backend (including OPFS SQLite)

Recommended structure:

- Add a small test-support crate or shared test module (e.g. `tax-repo-conformance`) exposing reusable async test functions.
- Each backend crate provides a backend-specific fixture/constructor and invokes the same conformance cases.
- Include conformance cases that verify portable ID semantics:
  - create preserves caller-provided UUIDv7 `id`
  - round-trip get/list returns the same `id`
  - import/upsert by `id` updates existing records instead of duplicating

3. WASM backend tests (`tax-db-browser`)

- Use `wasm-bindgen-test` for browser-executed repository tests (IndexedDB CRUD, timestamps, persistence after reload simulation where possible).
- Keep pure Rust unit tests for in-memory/reference-data mapping and serialization helpers.

4. `tax-ui` browser UI tests

- `wasm-bindgen-test` for Rust-side component/state tests where feasible.
- `Vitest` for any JS/TS helper code, test harness utilities, or browser glue that is introduced (optional if the UI remains fully Rust + Trunk with no meaningful JS helpers).
- `Playwright` for end-to-end browser flows:
  - load reference data
  - run calculation
  - create/update/delete estimate
  - reload page and verify IndexedDB persistence

5. Native backend tests (`tax-db-sqlite`)

- Keep existing integration tests as the authoritative SQL/schema behavior checks.
- Also run the shared repository conformance suite here to ensure parity with the browser backend contract.

6. CI execution matrix (minimum)

- Native: `cargo test --workspace`
- WASM logic check: `cargo check` for `wasm32-unknown-unknown`
- Browser repo tests: `wasm-bindgen-test` in headless browser (e.g. Chrome/Firefox)
- JS helper tests: `Vitest` (if JS/TS helper modules exist)
- End-to-end: `Playwright` (headless)

## Implementation Sequence (Recommended)

1. Refactor `tax-ui` into shared modules + native-only CLI modules (no behavior change).
2. Remove `tax-db-sqlite` registration/dependency from shared `tax-ui` code paths.
3. Change `tax_estimate.id` to a portable UUIDv7 ID across schema/models/repository APIs (breaking change while unpublished).
4. Make repository trait async bounds wasm-friendly if required.
5. Create canonical JSON reference data + generators (and a browser JSON loader path).
6. Implement `tax-db-browser` (IndexedDB + in-memory reference data) using the same UUIDv7 `id` as the record key.
7. Add a minimal WASM UI entry path inside `tax-ui` that can load reference data and run calculations.
8. Add browser persistence for `TaxEstimate` CRUD and export/import (upsert by portable `id`).
9. Add CI target checks for `wasm32-unknown-unknown` (`tax-core`, `tax-db-browser`, `tax-ui`).
10. Evaluate whether OPFS-backed SQLite is still needed.

## Risks and Mitigations

### Risk: Trait bounds (`Send`/`Sync`) block browser backend

Mitigation:

- Make bounds target-aware early, before writing the browser backend.

### Risk: Duplicated reference data definitions (SQL seeds vs browser data)

Mitigation:

- Introduce a single generation pipeline (preferably via `tax-data`) so browser artifacts are derived from the same source as native seeds.

### Risk: WASM bundle size grows quickly

Mitigation:

- Reuse `tax-core`, avoid wasm SQLite in MVP, and keep browser backend minimal.

### Risk: Browser persistence semantics differ from sqlite

Mitigation:

- Constrain browser backend scope initially to the required CRUD and lookup behaviors, with conformance tests.

### Risk: ID migration from current integer schema causes churn across crates

Mitigation:

- Do the UUIDv7 `id` migration early (before the WASM backend lands), while the app is unpublished.
- Update schema, `tax-core` models, and repository traits together in one breaking-change pass.
- Add conformance tests that lock down cross-backend ID behavior before implementing export/import UI.

## Summary Recommendation

- Yes, OPFS-backed SQLite is viable, but it is probably not the best first step for this workspace.
- The fastest and lowest-risk path is:
  - reuse `tax-core`
  - evolve `tax-ui` into a dual-target presentation crate
  - isolate native CLI and sqlite wiring behind native-only modules/features
  - add a browser repository backend using generated JSON reference assets + IndexedDB for estimates
  - add a WASM presentation path inside `tax-ui`
- Keep OPFS/SQLite as a second-stage enhancement if browser requirements outgrow the simpler backend.
