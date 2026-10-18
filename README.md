# 2049

A Blade Runner themed 2048 clone. Static site, no build step, tuned for mobile.

## Play

Swipe on the board or use the arrow keys (WASD and HJKL also work).

Two modes, switched with the control between UNDO and NEW RUN:

- **2048**: merge matching tiles until you reach 2048. `Z` undoes the last move,
  `R` starts a new run.
- **Collatz**: two equal tiles `n` become `n/2` when `n` is even and `3n+1` when
  `n` is odd. Two 1s vanish. No tiles spawn. Clear the board to finish the level.
  Boards are generated to be solvable and PAR is the exact shortest solution.
  The game tells you the moment a position can no longer be cleared. `Z` undoes
  (full history within a level), `R` retries the same board, `N` deals a new one.

## Files

- `index.html`, `style.css`, `game.js`: the game and both modes' UI.
- `collatz.js`: Collatz rules, exact solver and board generator (pure, no DOM).
- `test/collatz.test.js`: engine tests and generator statistics (`node test/collatz.test.js`).
- `favicon.svg`: tab icon.
- `.github/workflows/pages.yml`: deploys the repository root to GitHub Pages on
  every push to `main`.

## Hosting on GitHub Pages

1. In the repository settings open **Pages**.
2. Under **Build and deployment**, set **Source** to **GitHub Actions**.
3. Push to `main`. The workflow publishes the site at
   `https://<user>.github.io/2049/`.

## Local preview

Any static file server works, for example:

```sh
python3 -m http.server 8000
```

Then open <http://localhost:8000>.
