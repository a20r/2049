# 2049

A Blade Runner themed 2048 clone. Static site, no build step, tuned for mobile.

## Play

Swipe on the board or use the arrow keys (WASD and HJKL also work). Merge matching
tiles until you reach 2048. `Z` undoes the last move, `R` starts a new run.

## Files

- `index.html`, `style.css`, `game.js`: the whole game.
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
