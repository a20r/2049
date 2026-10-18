# 2049

A Blade Runner themed 2048 clone, written in Rust and compiled to WebAssembly.
Static site on GitHub Pages, tuned for mobile.

## Play

Swipe on the board or use the arrow keys (WASD and HJKL also work).

Four modes, switched with the control under the board:

- **2048**: merge matching tiles until you reach 2048. `Z` undoes the last move,
  `R` starts a new run.
- **Collatz**: two equal tiles `n` become `n/2` when `n` is even and `3n+1` when
  `n` is odd. Two 1s vanish. No tiles spawn. Clear the board to finish the level.
  Boards are generated to be solvable and PAR is the exact shortest solution.
  The game tells you the moment a position can no longer be cleared. `Z` undoes
  (full history within a level), `R` retries the same board, `N` deals a new one.
- **Daily**: one Collatz board per calendar day, the same for everyone, seeded
  from the date. Your best move count for the day and your streak of cleared
  days are kept, and SHARE copies a one-line result.
- **Endless**: Collatz rules with a tile spawning after every move. Keep the
  board clear for as long as you can; the score is tiles removed. `Z` undoes
  one move, `R` starts a new run.

## Layout

- `engine/`: the rules, exact solver, board generator and daily seeding as a
  pure Rust crate with no dependencies (`cargo test -p nexus-engine`).
- `src/`: the browser game: state, persistence, rendering and input through
  `web-sys`, compiled to WebAssembly with `wasm-bindgen`.
- `web/`: the static page, stylesheet and icon.
- `build.sh`: builds the module and assembles `dist/`.
- `.github/workflows/pages.yml`: tests, builds and deploys `dist/` to GitHub
  Pages on every push to `main`.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version "$(cargo metadata --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="wasm-bindgen"))')"
cargo test --workspace
./build.sh            # or ./build.sh debug
```

Then serve `dist/` with any static file server, for example:

```sh
cd dist && python3 -m http.server 8000
```

The server must send `.wasm` as `application/wasm` for streaming compilation;
GitHub Pages and Python's server both do.

## Hosting on GitHub Pages

1. In the repository settings open **Pages**.
2. Under **Build and deployment**, set **Source** to **GitHub Actions**.
3. Push to `main`. The workflow publishes the site at
   `https://<user>.github.io/2049/`.
