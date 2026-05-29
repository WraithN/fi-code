# Read / Glob / Grep GitIgnore Support Design

## Problem

- `read` lacks line-range parameters (`start`, `offset`)
- `glob` has no `.gitignore` awareness and currently returns files in `node_modules/`, `target/`, etc.
- `grep` recently added a hard-coded `BLOCKED_DIRS` blacklist; this should be replaced with `.gitignore` filtering

## Solution

### read

- Add parameters: `start` (optional, default 0), `offset` (optional)
- `offset` omitted = read from `start` to EOF
- Check if file is ignored by `.gitignore` before reading
- Still uses Rust built-in `BufReader` (rg is not suitable for reading files)

### glob

- **rg available**: `rg --files --glob <pattern>` (native `.gitignore` support)
- **rg unavailable**: `ignore::WalkBuilder` to traverse with `.gitignore` filtering
- Remove any hard-coded blacklist

### grep

- Remove `BLOCKED_DIRS` constant and `is_blocked_path` helper
- **rg path**: rg already respects `.gitignore`, no extra filtering needed
- **fallback path**: `ignore::WalkBuilder` for `.gitignore`-aware traversal

## Files Changed

- `crates/core/Cargo.toml` — add `ignore` crate
- `crates/core/src/tools/basic_tools.rs` — read/glob/grep implementations
- `crates/core/src/tools/mod.rs` — update read schema (add start/offset)
