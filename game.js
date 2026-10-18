(function () {
  'use strict';

  var SIZE = 4;
  var WIN_VALUE = 2048;
  var MOVE_MS = 150; // slightly longer than the CSS slide so the swap lands after it
  var STORAGE_KEY = 'nexus2049.state';
  var BEST_KEY = 'nexus2049.best';

  var boardEl = document.getElementById('board');
  var tilesEl = document.getElementById('tiles');
  var scoreEl = document.getElementById('score');
  var bestEl = document.getElementById('best');
  var deltaEl = document.getElementById('score-delta');
  var statusEl = document.getElementById('status');
  var undoBtn = document.getElementById('undo');
  var newBtn = document.getElementById('new-game');
  var overlayEl = document.getElementById('overlay');
  var overlayKicker = document.getElementById('overlay-kicker');
  var overlayTitle = document.getElementById('overlay-title');
  var overlaySub = document.getElementById('overlay-sub');
  var overlayContinue = document.getElementById('overlay-continue');
  var overlayRestart = document.getElementById('overlay-restart');

  var nextId = 1;
  var grid = [];          // grid[row][col] = tile object or null
  var score = 0;
  var best = 0;
  var won = false;        // reached WIN_VALUE at least once
  var keepPlaying = false;
  var over = false;
  var history = null;     // single-step undo snapshot
  var animating = false;
  var queuedDir = null;   // one move buffered while a slide is in flight
  var elements = {};      // tile id -> DOM element

  var LOSS_LINES = [
    'ALL THOSE MOMENTS WILL BE LOST IN TIME',
    'LIKE TEARS IN RAIN',
    'RETIREMENT COMPLETE',
    'YOUR INCEPT DATE HAS EXPIRED'
  ];

  // ---------- storage ----------

  function safeGet(key) {
    try { return localStorage.getItem(key); } catch (e) { return null; }
  }
  function safeSet(key, value) {
    try { localStorage.setItem(key, value); } catch (e) { /* ignore */ }
  }
  function safeRemove(key) {
    try { localStorage.removeItem(key); } catch (e) { /* ignore */ }
  }

  function serialize() {
    var cells = [];
    for (var r = 0; r < SIZE; r++) {
      for (var c = 0; c < SIZE; c++) {
        cells.push(grid[r][c] ? grid[r][c].value : 0);
      }
    }
    return { cells: cells, score: score, won: won, keepPlaying: keepPlaying, over: over };
  }

  function saveState() {
    if (over) {
      safeRemove(STORAGE_KEY);
    } else {
      safeSet(STORAGE_KEY, JSON.stringify(serialize()));
    }
  }

  function loadState() {
    var raw = safeGet(STORAGE_KEY);
    if (!raw) return false;
    try {
      var s = JSON.parse(raw);
      if (!s || !Array.isArray(s.cells) || s.cells.length !== SIZE * SIZE) return false;
      emptyGrid();
      for (var i = 0; i < s.cells.length; i++) {
        var v = s.cells[i];
        if (typeof v === 'number' && v > 0) {
          grid[Math.floor(i / SIZE)][i % SIZE] = makeTile(v, Math.floor(i / SIZE), i % SIZE);
        }
      }
      score = typeof s.score === 'number' ? s.score : 0;
      won = !!s.won;
      keepPlaying = !!s.keepPlaying;
      over = false;
      return true;
    } catch (e) {
      return false;
    }
  }

  // ---------- grid helpers ----------

  function makeTile(value, row, col) {
    return { id: nextId++, value: value, row: row, col: col };
  }

  function emptyGrid() {
    grid = [];
    for (var r = 0; r < SIZE; r++) {
      var row = [];
      for (var c = 0; c < SIZE; c++) row.push(null);
      grid.push(row);
    }
  }

  function emptyCells() {
    var out = [];
    for (var r = 0; r < SIZE; r++) {
      for (var c = 0; c < SIZE; c++) {
        if (!grid[r][c]) out.push({ row: r, col: c });
      }
    }
    return out;
  }

  function addRandomTile() {
    var cells = emptyCells();
    if (!cells.length) return null;
    var cell = cells[Math.floor(Math.random() * cells.length)];
    var tile = makeTile(Math.random() < 0.9 ? 2 : 4, cell.row, cell.col);
    grid[cell.row][cell.col] = tile;
    return tile;
  }

  function movesAvailable() {
    for (var r = 0; r < SIZE; r++) {
      for (var c = 0; c < SIZE; c++) {
        var t = grid[r][c];
        if (!t) return true;
        if (c + 1 < SIZE && grid[r][c + 1] && grid[r][c + 1].value === t.value) return true;
        if (r + 1 < SIZE && grid[r + 1][c] && grid[r + 1][c].value === t.value) return true;
      }
    }
    return false;
  }

  function snapshot() {
    var cells = [];
    for (var r = 0; r < SIZE; r++) {
      for (var c = 0; c < SIZE; c++) {
        var t = grid[r][c];
        cells.push(t ? { id: t.id, value: t.value } : null);
      }
    }
    return { cells: cells, score: score, won: won, keepPlaying: keepPlaying };
  }

  function restore(snap) {
    emptyGrid();
    for (var i = 0; i < snap.cells.length; i++) {
      var s = snap.cells[i];
      if (s) {
        var r = Math.floor(i / SIZE), c = i % SIZE;
        grid[r][c] = { id: s.id, value: s.value, row: r, col: c };
      }
    }
    score = snap.score;
    won = snap.won;
    keepPlaying = snap.keepPlaying;
    over = false;
  }

  // ---------- move logic ----------

  // Returns { moved, merges:[{tile, sources:[a,b]}], gained } and mutates grid.
  function move(dir) {
    // dir: 0 up, 1 right, 2 down, 3 left
    var dr = dir === 0 ? -1 : dir === 2 ? 1 : 0;
    var dc = dir === 3 ? -1 : dir === 1 ? 1 : 0;

    var rows = [], cols = [];
    for (var i = 0; i < SIZE; i++) { rows.push(i); cols.push(i); }
    if (dr === 1) rows.reverse();
    if (dc === 1) cols.reverse();

    var moved = false;
    var gained = 0;
    var merges = [];
    var mergedIds = {};

    for (var ri = 0; ri < SIZE; ri++) {
      for (var ci = 0; ci < SIZE; ci++) {
        var r = rows[ri], c = cols[ci];
        var tile = grid[r][c];
        if (!tile) continue;

        var nr = r, nc = c;
        while (true) {
          var tr = nr + dr, tc = nc + dc;
          if (tr < 0 || tr >= SIZE || tc < 0 || tc >= SIZE) break;
          if (grid[tr][tc]) break;
          nr = tr; nc = tc;
        }

        var ar = nr + dr, ac = nc + dc;
        var ahead = (ar >= 0 && ar < SIZE && ac >= 0 && ac < SIZE) ? grid[ar][ac] : null;

        if (ahead && ahead.value === tile.value && !mergedIds[ahead.id]) {
          // merge into ahead
          grid[r][c] = null;
          var merged = makeTile(tile.value * 2, ar, ac);
          grid[ar][ac] = merged;
          mergedIds[merged.id] = true;
          tile.row = ar; tile.col = ac;
          merges.push({ tile: merged, sources: [ahead, tile] });
          gained += merged.value;
          moved = true;
        } else if (nr !== r || nc !== c) {
          grid[r][c] = null;
          grid[nr][nc] = tile;
          tile.row = nr; tile.col = nc;
          moved = true;
        }
      }
    }

    return { moved: moved, merges: merges, gained: gained };
  }

  // ---------- rendering ----------

  function tileEl(tile) {
    var el = document.createElement('div');
    el.className = 'tile';
    el.dataset.v = String(tile.value);
    el.style.setProperty('--x', tile.col);
    el.style.setProperty('--y', tile.row);
    var v = document.createElement('span');
    v.className = 'v';
    v.textContent = tile.value;
    el.appendChild(v);
    elements[tile.id] = el;
    tilesEl.appendChild(el);
    return el;
  }

  function positionEl(tile) {
    var el = elements[tile.id];
    if (!el) return;
    el.style.setProperty('--x', tile.col);
    el.style.setProperty('--y', tile.row);
  }

  function removeEl(id) {
    var el = elements[id];
    if (el && el.parentNode) el.parentNode.removeChild(el);
    delete elements[id];
  }

  function renderAll() {
    tilesEl.innerHTML = '';
    elements = {};
    for (var r = 0; r < SIZE; r++) {
      for (var c = 0; c < SIZE; c++) {
        if (grid[r][c]) tileEl(grid[r][c]);
      }
    }
    renderScore();
  }

  function renderScore() {
    scoreEl.textContent = score;
    bestEl.textContent = best;
  }

  function showDelta(n) {
    if (!n) return;
    deltaEl.textContent = '+' + n;
    deltaEl.classList.remove('show');
    void deltaEl.offsetWidth; // restart animation
    deltaEl.classList.add('show');
  }

  function updateUndo() {
    undoBtn.disabled = !history || over;
  }

  function announce(msg) {
    statusEl.textContent = msg;
  }

  // ---------- overlays ----------

  function showOverlay(kind) {
    overlayEl.hidden = false;
    overlayEl.classList.toggle('win', kind === 'win');
    if (kind === 'win') {
      overlayKicker.textContent = 'BASELINE TEST PASSED';
      overlayTitle.textContent = 'MORE HUMAN THAN HUMAN';
      overlaySub.textContent = 'YOU REACHED 2048. TYRELL WOULD BE PROUD.';
      overlayContinue.hidden = false;
      announce('You reached 2048. More human than human.');
    } else {
      overlayKicker.textContent = 'RETIRED';
      overlayTitle.textContent = 'TIME TO DIE';
      overlaySub.textContent = LOSS_LINES[Math.floor(Math.random() * LOSS_LINES.length)] + ' · SCORE ' + score;
      overlayContinue.hidden = true;
      announce('Game over. Final score ' + score + '.');
    }
  }

  function hideOverlay() {
    overlayEl.hidden = true;
  }

  // ---------- game flow ----------

  function newGame() {
    emptyGrid();
    score = 0;
    won = false;
    keepPlaying = false;
    over = false;
    history = null;
    animating = false;
    queuedDir = null;
    hideOverlay();
    addRandomTile();
    addRandomTile();
    renderAll();
    updateUndo();
    saveState();
    announce('New game started.');
  }

  function undo() {
    if (!history || animating) return;
    restore(history);
    history = null;
    queuedDir = null;
    hideOverlay();
    renderAll();
    updateUndo();
    saveState();
    announce('Move undone.');
  }

  function handleMove(dir) {
    if (over || !overlayEl.hidden) return;
    if (animating) { queuedDir = dir; return; }

    var snap = snapshot();
    var result = move(dir);

    if (!result.moved) {
      boardEl.classList.remove('shake');
      void boardEl.offsetWidth;
      boardEl.classList.add('shake');
      return;
    }

    history = snap;
    animating = true;

    // Slide everything (including tiles about to be merged) to its target.
    for (var r = 0; r < SIZE; r++) {
      for (var c = 0; c < SIZE; c++) {
        var t = grid[r][c];
        if (t) positionEl(t);
      }
    }
    result.merges.forEach(function (m) {
      m.sources.forEach(positionEl);
    });

    score += result.gained;
    if (score > best) {
      best = score;
      safeSet(BEST_KEY, String(best));
    }

    var reachedWin = false;
    result.merges.forEach(function (m) {
      if (m.tile.value >= WIN_VALUE && !won) reachedWin = true;
    });

    setTimeout(function () {
      // Swap merge sources for merged tiles, then spawn a new tile.
      result.merges.forEach(function (m) {
        m.sources.forEach(function (s) { removeEl(s.id); });
        var el = tileEl(m.tile);
        el.classList.add('merged');
      });

      var spawned = addRandomTile();
      if (spawned) tileEl(spawned).classList.add('new');

      renderScore();
      showDelta(result.gained);

      if (reachedWin) {
        won = true;
        if (!keepPlaying) showOverlay('win');
      }

      if (!movesAvailable()) {
        over = true;
        showOverlay('lose');
      }

      animating = false;
      updateUndo();
      saveState();

      if (queuedDir !== null) {
        var next = queuedDir;
        queuedDir = null;
        handleMove(next);
      }
    }, MOVE_MS);
  }

  // ---------- input ----------

  var KEYS = {
    ArrowUp: 0, ArrowRight: 1, ArrowDown: 2, ArrowLeft: 3,
    w: 0, d: 1, s: 2, a: 3, W: 0, D: 1, S: 2, A: 3,
    k: 0, l: 1, j: 2, h: 3
  };

  document.addEventListener('keydown', function (e) {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    var dir = KEYS[e.key];
    if (dir === undefined) {
      if ((e.key === 'z' || e.key === 'Z') && !e.shiftKey) { undo(); e.preventDefault(); }
      if (e.key === 'r' || e.key === 'R') { newGame(); e.preventDefault(); }
      return;
    }
    e.preventDefault();
    handleMove(dir);
  });

  // Touch and pointer swipe.
  var touchStartX = 0, touchStartY = 0, touching = false;
  var SWIPE_MIN = 24;

  function onStart(x, y) {
    touchStartX = x; touchStartY = y; touching = true;
  }
  // Returns a direction once the gesture passes the threshold, otherwise null.
  function swipeDir(x, y) {
    var dx = x - touchStartX, dy = y - touchStartY;
    var ax = Math.abs(dx), ay = Math.abs(dy);
    if (Math.max(ax, ay) < SWIPE_MIN) return null;
    if (ax > ay) return dx > 0 ? 1 : 3;
    return dy > 0 ? 2 : 0;
  }
  function onMove(x, y) {
    if (!touching) return;
    var dir = swipeDir(x, y);
    if (dir === null) return;
    touching = false;
    handleMove(dir);
  }
  function onEnd(x, y) {
    onMove(x, y);
    touching = false;
  }

  boardEl.addEventListener('touchstart', function (e) {
    if (e.touches.length !== 1) return;
    if (e.target.closest('button')) return;
    onStart(e.touches[0].clientX, e.touches[0].clientY);
  }, { passive: true });

  boardEl.addEventListener('touchmove', function (e) {
    if (!touching) return;
    e.preventDefault();
    // Fire as soon as the swipe is unambiguous so the slide starts under the finger.
    if (e.touches.length === 1) onMove(e.touches[0].clientX, e.touches[0].clientY);
  }, { passive: false });

  boardEl.addEventListener('touchend', function (e) {
    if (!e.changedTouches.length) return;
    onEnd(e.changedTouches[0].clientX, e.changedTouches[0].clientY);
  }, { passive: true });

  boardEl.addEventListener('touchcancel', function () { touching = false; }, { passive: true });

  boardEl.addEventListener('mousedown', function (e) {
    if (e.button !== 0 || e.target.closest('button')) return;
    onStart(e.clientX, e.clientY);
  });
  document.addEventListener('mouseup', function (e) {
    if (touching) onEnd(e.clientX, e.clientY);
  });

  undoBtn.addEventListener('click', undo);
  newBtn.addEventListener('click', newGame);
  overlayRestart.addEventListener('click', newGame);
  overlayContinue.addEventListener('click', function () {
    keepPlaying = true;
    hideOverlay();
    saveState();
  });

  // ---------- boot ----------

  best = parseInt(safeGet(BEST_KEY), 10) || 0;
  if (loadState()) {
    renderAll();
    updateUndo();
    if (won && !keepPlaying) showOverlay('win');
  } else {
    newGame();
  }
})();
