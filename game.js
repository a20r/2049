(function () {
  'use strict';

  var SIZE = 4;
  var WIN_VALUE = 2048;
  var MOVE_MS = 150; // slightly longer than the CSS slide so the swap lands after it
  var UNDO_CAP = 100;

  var STORAGE_KEY = 'nexus2049.state';
  var BEST_KEY = 'nexus2049.best';
  var MODE_KEY = 'nexus2049.mode';
  var CZ_KEY = 'nexus2049.collatz.state';
  var CZ_BEST_KEY = 'nexus2049.collatz.best';
  var CZ_INTRO_KEY = 'nexus2049.collatz.intro';
  var DAILY_KEY = 'nexus2049.daily.state';
  var DAILY_HIST_KEY = 'nexus2049.daily.history'; // { 'YYYY-MM-DD': best moves }

  var hasCollatz = typeof window.Collatz !== 'undefined';

  var boardEl = document.getElementById('board');
  var tilesEl = document.getElementById('tiles');
  var scoreEl = document.getElementById('score');
  var bestEl = document.getElementById('best');
  var scoreLabelEl = document.getElementById('score-label');
  var bestLabelEl = document.getElementById('best-label');
  var deltaEl = document.getElementById('score-delta');
  var statusEl = document.getElementById('status');
  var statusLineEl = document.getElementById('status-line');
  var taglineEl = document.getElementById('tagline');
  var undoBtn = document.getElementById('undo');
  var newBtn = document.getElementById('new-game');
  var mode2048Btn = document.getElementById('mode-2048');
  var modeCollatzBtn = document.getElementById('mode-collatz');
  var modeDailyBtn = document.getElementById('mode-daily');
  var overlayEl = document.getElementById('overlay');
  var overlayKicker = document.getElementById('overlay-kicker');
  var overlayTitle = document.getElementById('overlay-title');
  var overlaySub = document.getElementById('overlay-sub');
  var overlayPrimary = document.getElementById('overlay-primary');
  var overlaySecondary = document.getElementById('overlay-secondary');
  var overlayAlt = document.getElementById('overlay-alt');

  var mode = '2048';      // '2048', 'collatz' or 'daily'
  var nextId = 1;
  var grid = [];          // grid[row][col] = tile object or null
  var elements = {};      // tile id -> DOM element
  var animating = false;
  var queuedDir = null;   // one move buffered while a slide is in flight
  var moveTimer = null;
  var pendingFinish = null; // the function the move timer will run
  var memState = {};      // in-memory saves per mode, so a mode switch is lossless

  // 2048 state
  var score = 0;
  var best = 0;
  var won = false;
  var keepPlaying = false;
  var over = false;
  var history = null;     // single-step undo snapshot

  // Collatz state, shared by the collatz and daily modes
  var cz = null;          // { level, date, moves, par, start, undo, undos, stuck, cleared }
  var czBest = 0;

  var LOSS_LINES = [
    'ALL THOSE MOMENTS WILL BE LOST IN TIME',
    'LIKE TEARS IN RAIN',
    'RETIREMENT COMPLETE',
    'YOUR INCEPT DATE HAS EXPIRED'
  ];

  // Both Collatz variants share rules, rendering, undo and overlays.
  function isCz() {
    return mode === 'collatz' || mode === 'daily';
  }

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

  function validCells(a) {
    if (!Array.isArray(a) || a.length !== SIZE * SIZE) return false;
    for (var i = 0; i < a.length; i++) {
      if (typeof a[i] !== 'number' || a[i] < 0 || a[i] !== Math.floor(a[i])) return false;
    }
    return true;
  }

  // ---------- grid helpers ----------

  function makeTile(value, row, col) {
    return { id: nextId++, value: value, row: row, col: col };
  }

  function newEmptyGrid() {
    var g = [];
    for (var r = 0; r < SIZE; r++) {
      var row = [];
      for (var c = 0; c < SIZE; c++) row.push(null);
      g.push(row);
    }
    return g;
  }

  function emptyGrid() {
    grid = newEmptyGrid();
  }

  function cellsFromGrid() {
    var cells = [];
    for (var r = 0; r < SIZE; r++) {
      for (var c = 0; c < SIZE; c++) cells.push(grid[r][c] ? grid[r][c].value : 0);
    }
    return cells;
  }

  function gridFromCells(cells) {
    var g = newEmptyGrid();
    for (var i = 0; i < cells.length; i++) {
      if (cells[i] > 0) g[Math.floor(i / SIZE)][i % SIZE] = makeTile(cells[i], Math.floor(i / SIZE), i % SIZE);
    }
    return g;
  }

  function countTiles() {
    var n = 0;
    for (var r = 0; r < SIZE; r++) for (var c = 0; c < SIZE; c++) if (grid[r][c]) n++;
    return n;
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

  // ---------- 2048 state ----------

  function serialize() {
    return { cells: cellsFromGrid(), score: score, won: won, keepPlaying: keepPlaying, over: over };
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

  function save2048() {
    if (over) {
      memState['2048'] = null;
      safeRemove(STORAGE_KEY);
    } else {
      memState['2048'] = { state: serialize(), history: history };
      safeSet(STORAGE_KEY, JSON.stringify(serialize()));
    }
  }

  function apply2048(s) {
    if (!s || !validCells(s.cells)) return false;
    grid = gridFromCells(s.cells);
    score = typeof s.score === 'number' ? s.score : 0;
    won = !!s.won;
    keepPlaying = !!s.keepPlaying;
    over = false;
    return true;
  }

  function load2048() {
    var mem = memState['2048'];
    if (mem && apply2048(mem.state)) {
      history = mem.history || null;
      return true;
    }
    var raw = safeGet(STORAGE_KEY);
    if (!raw) return false;
    try {
      if (!apply2048(JSON.parse(raw))) return false;
      history = null;
      return true;
    } catch (e) {
      return false;
    }
  }

  // ---------- Collatz state ----------

  function serializeCz() {
    return {
      v: 1,
      level: cz.level,
      date: cz.date,
      moves: cz.moves,
      par: cz.par,
      start: cz.start,
      cells: cellsFromGrid(),
      undo: cz.undo,
      undos: cz.undos,
      stuck: cz.stuck,
      cleared: cz.cleared
    };
  }

  function czKey() {
    return mode === 'daily' ? DAILY_KEY : CZ_KEY;
  }

  function saveCz() {
    var s = serializeCz();
    memState[mode] = s;
    safeSet(czKey(), JSON.stringify(s));
  }

  function applyCz(s) {
    if (!s || s.v !== 1) return false;
    if (!validCells(s.cells) || !validCells(s.start)) return false;
    if (typeof s.level !== 'number' || s.level < 1) return false;
    var undo = [];
    if (Array.isArray(s.undo)) {
      for (var i = 0; i < s.undo.length; i++) {
        var u = s.undo[i];
        if (u && validCells(u.cells) && typeof u.moves === 'number') undo.push({ cells: u.cells, moves: u.moves });
      }
    }
    if (undo.length > UNDO_CAP) undo = undo.slice(undo.length - UNDO_CAP);
    cz = {
      level: Math.floor(s.level),
      date: typeof s.date === 'string' ? s.date : null,
      moves: typeof s.moves === 'number' ? s.moves : 0,
      par: typeof s.par === 'number' ? s.par : 0,
      start: s.start.slice(),
      undo: undo,
      undos: typeof s.undos === 'number' ? s.undos : 0,
      stuck: !!s.stuck,
      cleared: !!s.cleared
    };
    grid = gridFromCells(s.cells);
    return true;
  }

  // A daily save only counts for the day it was made.
  function applyCzForMode(s) {
    if (!applyCz(s)) return false;
    if (mode === 'daily' && cz.date !== Collatz.dateKey()) return false;
    return true;
  }

  function loadCz() {
    if (memState[mode] && applyCzForMode(memState[mode])) return true;
    var raw = safeGet(czKey());
    if (!raw) return false;
    try {
      return applyCzForMode(JSON.parse(raw));
    } catch (e) {
      return false;
    }
  }

  function loadHistory() {
    try {
      var h = JSON.parse(safeGet(DAILY_HIST_KEY) || '{}');
      return (h && typeof h === 'object' && !Array.isArray(h)) ? h : {};
    } catch (e) {
      return {};
    }
  }

  // Consecutive cleared days ending on `key`.
  function streakEnding(hist, key) {
    var parts = key.split('-');
    var d = new Date(+parts[0], +parts[1] - 1, +parts[2]);
    var n = 0;
    while (typeof hist[Collatz.dateKey(d)] === 'number') {
      n++;
      d.setDate(d.getDate() - 1);
      if (n > 10000) break;
    }
    return n;
  }

  function saveState() {
    if (isCz()) saveCz(); else save2048();
  }

  // ---------- 2048 move logic ----------

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

  function bandFor(value) {
    if (value < 4) return 0;
    if (value < 16) return 1;
    if (value < 64) return 2;
    return 3;
  }

  function tileEl(tile) {
    var el = document.createElement('div');
    el.className = 'tile';
    el.style.setProperty('--x', tile.col);
    el.style.setProperty('--y', tile.row);
    var v = document.createElement('span');
    v.className = 'v';
    v.textContent = tile.value;
    el.appendChild(v);
    if (isCz()) {
      el.dataset.par = tile.value === 1 ? 'one' : (tile.value % 2 ? 'odd' : 'even');
      el.dataset.band = String(bandFor(tile.value));
      el.dataset.len = String(String(tile.value).length);
      var cap = document.createElement('span');
      cap.className = 'cap';
      cap.textContent = tile.value === 1 ? 'CLEAR' : '→' + Collatz.f(tile.value);
      el.appendChild(cap);
    } else {
      el.dataset.v = String(tile.value);
    }
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
    if (mode === 'daily') {
      scoreEl.textContent = cz ? '#' + cz.level : '';
      bestEl.textContent = cz ? cz.moves : 0;
    } else if (mode === 'collatz') {
      scoreEl.textContent = cz ? cz.level : 1;
      bestEl.textContent = cz ? cz.moves : 0;
    } else {
      scoreEl.textContent = score;
      bestEl.textContent = best;
    }
  }

  function renderStatus() {
    var text, cls = '';
    if (isCz() && cz) {
      var n = countTiles();
      var left = n + (n === 1 ? ' TILE LEFT' : ' TILES LEFT');
      if (cz.stuck) {
        text = 'NO PATH · UNDO TO CONTINUE';
        cls = 'warn';
      } else if (cz.cleared && mode === 'daily') {
        var hist = loadHistory();
        text = 'CLEARED IN ' + cz.moves + ' · BEST ' + hist[cz.date] + ' · STREAK ' + streakEnding(hist, cz.date);
        cls = 'goal';
      } else if (cz.cleared) {
        text = 'BOARD CLEARED · BEST LEVEL ' + czBest;
        cls = 'goal';
      } else if (mode === 'daily') {
        text = 'DAILY #' + cz.level + ' · ' + left + ' · PAR ' + cz.par;
        cls = 'goal';
      } else {
        text = 'CLEAR THE BOARD · ' + left + ' · PAR ' + cz.par;
        cls = 'goal';
      }
    } else {
      text = 'SWIPE OR USE ARROW KEYS';
    }
    statusLineEl.textContent = text;
    statusLineEl.className = 'status-line' + (cls ? ' ' + cls : '');
  }

  function showDelta(n) {
    if (!n) return;
    deltaEl.textContent = '+' + n;
    deltaEl.classList.remove('show');
    void deltaEl.offsetWidth; // restart animation
    deltaEl.classList.add('show');
  }

  function updateUndo() {
    if (isCz()) {
      undoBtn.disabled = !cz || cz.undo.length === 0;
    } else {
      undoBtn.disabled = !history || over;
    }
  }

  function announce(msg) {
    statusEl.textContent = msg;
  }

  function shake() {
    boardEl.classList.remove('shake');
    void boardEl.offsetWidth;
    boardEl.classList.add('shake');
  }

  // ---------- overlays ----------

  function setOverlayBtn(btn, spec) {
    if (!spec) {
      btn.hidden = true;
      btn.onclick = null;
      return;
    }
    btn.hidden = false;
    btn.textContent = spec.label;
    btn.onclick = spec.action;
  }

  function showOverlay(opts) {
    overlayEl.hidden = false;
    overlayEl.classList.toggle('win', !!opts.win);
    overlayEl.classList.toggle('collatz', isCz());
    overlayKicker.textContent = opts.kicker || '';
    overlayTitle.textContent = opts.title || '';
    overlaySub.textContent = opts.sub || '';
    overlaySub.classList.toggle('wide', !!opts.wide);
    setOverlayBtn(overlayPrimary, opts.primary);
    setOverlayBtn(overlaySecondary, opts.secondary);
    setOverlayBtn(overlayAlt, opts.alt);
    if (opts.announce) announce(opts.announce);
  }

  function hideOverlay() {
    overlayEl.hidden = true;
  }

  function showWin2048() {
    showOverlay({
      win: true,
      kicker: 'BASELINE TEST PASSED',
      title: 'MORE HUMAN THAN HUMAN',
      sub: 'YOU REACHED 2048. TYRELL WOULD BE PROUD.',
      secondary: { label: 'KEEP GOING', action: function () { keepPlaying = true; hideOverlay(); saveState(); } },
      primary: { label: 'NEW RUN', action: newGame },
      announce: 'You reached 2048. More human than human.'
    });
  }

  function showLose2048() {
    showOverlay({
      kicker: 'RETIRED',
      title: 'TIME TO DIE',
      sub: LOSS_LINES[Math.floor(Math.random() * LOSS_LINES.length)] + ' · SCORE ' + score,
      primary: { label: 'NEW RUN', action: newGame },
      announce: 'Game over. Final score ' + score + '.'
    });
  }

  // ---------- move completion ----------

  // Runs the pending end-of-move work now instead of when its timer fires, so
  // a new game, an undo or a mode switch never races a slide in flight.
  function flushMove() {
    if (!pendingFinish) return;
    clearTimeout(moveTimer);
    var fn = pendingFinish;
    pendingFinish = null;
    queuedDir = null;
    fn();
  }

  function scheduleFinish(fn) {
    pendingFinish = function () {
      pendingFinish = null;
      fn();
    };
    moveTimer = setTimeout(pendingFinish, MOVE_MS);
  }

  // ---------- 2048 game flow ----------

  function newGame() {
    flushMove();
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
    renderStatus();
    updateUndo();
    saveState();
    announce('New game started.');
  }

  function undo2048() {
    flushMove();
    if (!history) return;
    restore(history);
    history = null;
    hideOverlay();
    renderAll();
    updateUndo();
    saveState();
    announce('Move undone.');
  }

  function handleMove2048(dir) {
    var snap = snapshot();
    var result = move(dir);

    if (!result.moved) {
      shake();
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

    scheduleFinish(function () {
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
        if (!keepPlaying) showWin2048();
      }

      if (!movesAvailable()) {
        over = true;
        showLose2048();
      }

      animating = false;
      updateUndo();
      saveState();

      if (queuedDir !== null) {
        var next = queuedDir;
        queuedDir = null;
        handleMove(next);
      }
    });
  }

  // ---------- Collatz game flow ----------

  // Puts a fresh board on screen and resets the per-board state.
  function beginBoard(level, date, cells, par) {
    flushMove();
    cz = { level: level, date: date, moves: 0, par: par, start: cells.slice(), undo: [], undos: 0, stuck: false, cleared: false };
    grid = gridFromCells(cells);
    animating = false;
    queuedDir = null;
    hideOverlay();
    boardEl.classList.remove('stuck');
    renderAll();
    renderStatus();
    updateUndo();
    saveState();
    announce((mode === 'daily' ? 'Daily board ' + level : 'Level ' + level) + '. ' + countTiles() + ' tiles. Par ' + par + '.');
  }

  function startLevel(level, startCells) {
    var cells, par;
    if (startCells) {
      cells = startCells.slice();
      var sol = Collatz.solve(cells);
      par = sol.solved ? sol.moves.length : 0;
    } else {
      var g = Collatz.generate(level);
      cells = g.cells;
      par = g.par;
    }
    beginBoard(level, null, cells, par);
  }

  function startDaily() {
    var key = Collatz.dateKey();
    var g = Collatz.daily(key);
    beginBoard(g.day, key, g.cells, g.par);
  }

  // The same board again, from the top.
  function restartBoard() {
    beginBoard(cz.level, cz.date, cz.start, cz.par);
  }

  function retryLevel() {
    restartBoard();
  }

  function newBoard() {
    if (mode === 'daily') restartBoard(); else startLevel(cz.level);
  }

  function shareText() {
    var tier = tierFor();
    return '2049 DAILY #' + cz.level + ' · ' + cz.moves + ' MOVES · PAR ' + cz.par + ' · ' + tier + '\n' +
      location.origin + location.pathname;
  }

  function shareDaily() {
    var text = shareText();
    var btn = overlayPrimary;
    function done(label) { btn.textContent = label; }
    if (navigator.share) {
      navigator.share({ text: text }).then(function () { done('SHARED'); }, function () { /* cancelled */ });
      return;
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(function () { done('COPIED'); }, function () { done('COPY FAILED'); });
      return;
    }
    window.prompt('Copy your result', text);
  }

  function tierFor() {
    return cz.moves <= cz.par ? 'PERFECT' : cz.moves <= cz.par + 2 ? 'CLEAN' : 'CLEARED';
  }

  function undoCz() {
    flushMove();
    if (!cz.undo.length) return;
    var snap = cz.undo.pop();
    cz.moves = snap.moves;
    cz.undos++;
    cz.stuck = false;
    cz.cleared = false;
    grid = gridFromCells(snap.cells);
    hideOverlay();
    boardEl.classList.remove('stuck');
    renderAll();
    renderStatus();
    updateUndo();
    saveState();
    announce('Move undone.');
  }

  function showIntro() {
    showOverlay({
      win: true,
      kicker: 'NEW MODE',
      title: 'COLLATZ',
      sub: 'PAIR EQUAL TILES. EVEN HALVES. ODD BECOMES 3N+1. PAIR 1s TO CLEAR. EMPTY THE BOARD.',
      wide: true,
      primary: { label: 'BEGIN', action: function () { safeSet(CZ_INTRO_KEY, '1'); hideOverlay(); } },
      announce: 'Collatz mode. Pair equal tiles: even halves, odd becomes 3n plus 1, two 1s clear. Empty the board.'
    });
  }

  function showStuck() {
    boardEl.classList.add('stuck');
    showOverlay({
      kicker: 'BASELINE DEVIATION',
      title: 'NO PATH',
      sub: 'THESE TILES CAN NO LONGER PAIR OFF',
      primary: { label: 'UNDO', action: undoCz },
      secondary: { label: 'RETRY', action: retryLevel },
      alt: mode === 'daily' ? null : { label: 'NEW BOARD', action: newBoard },
      announce: 'No path to clear the board. Undo, retry' + (mode === 'daily' ? '.' : ', or take a new board.')
    });
  }

  function showCleared() {
    var tier = tierFor();
    var sub, opts;
    if (mode === 'daily') {
      var hist = loadHistory();
      sub = 'DAILY #' + cz.level + ' · ' + cz.moves + ' MOVES · PAR ' + cz.par;
      if (typeof hist[cz.date] === 'number' && hist[cz.date] < cz.moves) sub += ' · BEST ' + hist[cz.date];
      var streak = streakEnding(hist, cz.date);
      if (streak > 1) sub += ' · STREAK ' + streak;
      opts = {
        primary: { label: 'SHARE', action: shareDaily },
        secondary: { label: 'RETRY', action: restartBoard },
        announce: 'Daily board ' + cz.level + ' cleared in ' + cz.moves + ' moves, par ' + cz.par + '.'
      };
    } else {
      sub = 'LEVEL ' + cz.level + ' · ' + cz.moves + ' MOVES · PAR ' + cz.par;
      if (cz.undos) sub += ' · ' + cz.undos + (cz.undos === 1 ? ' UNDO' : ' UNDOS');
      opts = {
        primary: { label: 'NEXT LEVEL', action: function () { startLevel(cz.level + 1); } },
        announce: 'Level ' + cz.level + ' cleared in ' + cz.moves + ' moves, par ' + cz.par + '.'
      };
    }
    opts.win = true;
    opts.kicker = tier;
    opts.title = 'INTERLINKED';
    opts.sub = sub;
    showOverlay(opts);
  }

  function levelCleared() {
    cz.cleared = true;
    if (mode === 'daily') {
      var hist = loadHistory();
      if (typeof hist[cz.date] !== 'number' || cz.moves < hist[cz.date]) hist[cz.date] = cz.moves;
      safeSet(DAILY_HIST_KEY, JSON.stringify(hist));
    } else if (cz.level > czBest) {
      czBest = cz.level;
      safeSet(CZ_BEST_KEY, String(czBest));
    }
    showCleared();
  }

  function handleMoveCz(dir) {
    var before = cellsFromGrid();
    var res = Collatz.applyMove(before, dir, true);
    if (!res.moved) {
      shake();
      return;
    }

    cz.undo.push({ cells: before, moves: cz.moves });
    if (cz.undo.length > UNDO_CAP) cz.undo.shift();
    cz.moves++;
    animating = true;

    // Map the engine's source indices back to tile objects for animation.
    var byIdx = [];
    var r, c, i;
    for (r = 0; r < SIZE; r++) for (c = 0; c < SIZE; c++) byIdx.push(grid[r][c]);

    var newGrid = newEmptyGrid();
    var merges = [];
    var clears = [];
    for (i = 0; i < SIZE * SIZE; i++) {
      var s = res.sources[i];
      if (!s) continue;
      r = Math.floor(i / SIZE);
      c = i % SIZE;
      var a = byIdx[s[0]];
      a.row = r; a.col = c;
      positionEl(a);
      if (s.length === 1) {
        newGrid[r][c] = a;
      } else {
        var b = byIdx[s[1]];
        b.row = r; b.col = c;
        positionEl(b);
        var merged = makeTile(res.cells[i], r, c);
        newGrid[r][c] = merged;
        merges.push({ tile: merged, sources: [a, b] });
      }
    }
    res.clears.forEach(function (cl) {
      var cr = Math.floor(cl.at / SIZE), cc = cl.at % SIZE;
      var ta = byIdx[cl.a], tb = byIdx[cl.b];
      ta.row = cr; ta.col = cc; tb.row = cr; tb.col = cc;
      positionEl(ta);
      positionEl(tb);
      clears.push({ a: ta, b: tb });
    });
    grid = newGrid;

    scheduleFinish(function () {
      merges.forEach(function (m) {
        m.sources.forEach(function (src) { removeEl(src.id); });
        tileEl(m.tile).classList.add('merged');
      });
      clears.forEach(function (cl) {
        removeEl(cl.b.id);
        var el = elements[cl.a.id];
        delete elements[cl.a.id];
        if (el) {
          el.classList.add('cleared');
          setTimeout(function () { if (el.parentNode) el.parentNode.removeChild(el); }, 420);
        }
      });

      renderScore();

      var cells = cellsFromGrid();
      if (Collatz.isEmpty(cells)) {
        levelCleared();
      } else {
        var sol = Collatz.solve(cells);
        if (!sol.solved && !sol.exhausted) {
          cz.stuck = true;
          showStuck();
        }
      }
      renderStatus();

      animating = false;
      updateUndo();
      saveState();

      if (queuedDir !== null && !cz.stuck && !cz.cleared) {
        var next = queuedDir;
        queuedDir = null;
        handleMove(next);
      } else {
        queuedDir = null;
      }
    });
  }

  // ---------- dispatch ----------

  function handleMove(dir) {
    if (!overlayEl.hidden) return;
    if (mode === '2048' && over) return;
    if (animating) { queuedDir = dir; return; }
    if (isCz()) handleMoveCz(dir); else handleMove2048(dir);
  }

  function undo() {
    if (isCz()) undoCz(); else undo2048();
  }

  function primaryAction() {
    if (isCz()) newBoard(); else newGame();
  }

  // ---------- mode switch ----------

  function setMode(m, boot) {
    if (m !== 'collatz' && m !== 'daily') m = '2048';
    if (m !== '2048' && !hasCollatz) m = '2048';
    if (!boot) {
      if (m === mode) return;
      flushMove();
      saveState();
    }
    mode = m;
    safeSet(MODE_KEY, m);
    animating = false;
    queuedDir = null;

    var czMode = isCz();
    var daily = m === 'daily';
    mode2048Btn.setAttribute('aria-checked', m === '2048' ? 'true' : 'false');
    modeCollatzBtn.setAttribute('aria-checked', m === 'collatz' ? 'true' : 'false');
    modeDailyBtn.setAttribute('aria-checked', daily ? 'true' : 'false');
    boardEl.classList.toggle('collatz', czMode);
    boardEl.classList.remove('stuck');
    taglineEl.textContent = daily ? 'DAILY BASELINE TEST' : czMode ? 'CELLS INTERLINKED' : 'MORE HUMAN THAN HUMAN';
    scoreLabelEl.textContent = daily ? 'DAY' : czMode ? 'LEVEL' : 'SCORE';
    bestLabelEl.textContent = czMode ? 'MOVES' : 'BEST';
    newBtn.textContent = daily ? 'RETRY' : czMode ? 'NEW BOARD' : 'NEW RUN';
    hideOverlay();

    if (czMode) {
      if (loadCz()) {
        renderAll();
        if (cz.cleared) showCleared();
        else if (cz.stuck) showStuck();
      } else if (daily) {
        startDaily();
      } else {
        startLevel(1);
      }
      if (!safeGet(CZ_INTRO_KEY)) showIntro();
    } else {
      if (load2048()) {
        renderAll();
        if (won && !keepPlaying) showWin2048();
      } else {
        newGame();
      }
    }
    renderStatus();
    updateUndo();
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
      if (e.key === 'r' || e.key === 'R') {
        if (isCz()) retryLevel(); else newGame();
        e.preventDefault();
      }
      if (e.key === 'n' || e.key === 'N') { primaryAction(); e.preventDefault(); }
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
  newBtn.addEventListener('click', primaryAction);
  mode2048Btn.addEventListener('click', function () { setMode('2048'); });
  modeCollatzBtn.addEventListener('click', function () { setMode('collatz'); });
  modeDailyBtn.addEventListener('click', function () { setMode('daily'); });
  if (!hasCollatz) {
    modeCollatzBtn.disabled = true;
    modeDailyBtn.disabled = true;
  }

  // ---------- boot ----------

  best = parseInt(safeGet(BEST_KEY), 10) || 0;
  czBest = parseInt(safeGet(CZ_BEST_KEY), 10) || 0;
  setMode(safeGet(MODE_KEY) || '2048', true);
})();
