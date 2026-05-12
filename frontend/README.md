# fi-code Desktop

Tauri-based desktop application for fi-code.

## Development

```bash
# Terminal 1: Build the Rust CLI (required as sidecar)
cd ..
cargo build

# Terminal 2: Run desktop in dev mode
cd desktop
npm install
npm run tauri dev
```

## Build

```bash
cd desktop
npm run tauri build
```

Output will be in `src-tauri/target/release/bundle/`.

## Architecture

- **Frontend**: React 18 + TypeScript + Tailwind CSS + Zustand
- **Desktop Framework**: Tauri v2
- **Backend**: Existing fi-code Rust CLI running as sidecar (standalone mode) or remote HTTP server
- **Communication**: HTTP REST/JSON-RPC/SSE over localhost:4040
