/*
 * Collatz mode: rules, solver and board generator for 2049.
 *
 * Pure ES5, no DOM, no Node dependencies. Attaches to window.Collatz in a
 * browser and to module.exports under Node so the same file is testable.
 *
 * Board representation everywhere in this file: a flat array of 16 integers,
 * index = row * 4 + col, 0 = empty. Directions: 0 up, 1 right, 2 down, 3 left
 * (same as game.js).
 *
 * Merge rule: two equal tiles n become n/2 (even) or 3n+1 (odd, n >= 3).
 * Two 1s clear. A cleared pair keeps its cell blocked until the move ends,
 * exactly like a merged tile in 2048, so a trailing tile stops behind it:
 * [1,1,1,_] swiped left becomes [_,1,_,_].
 */
(function () {
  'use strict';

  var SIZE = 4;
  var CELLS = SIZE * SIZE;
  var DEFAULT_NODES = 40000;
  var GHOST = -1; // a cleared pair's cell for the rest of the move

  // ---------- the rule ----------

  // Merge result for two tiles of value n. 0 means the pair is cleared.
  function f(n) {
    if (n <= 1) return 0;
    return (n % 2 === 0) ? n / 2 : 3 * n + 1;
  }

  // ---------- Collatz distance ----------
  // dist(v) = number of merges a pair at v is away from the clearing pair at 1
  // (dist(1) = 0, dist(2) = 1, dist(4) = 2, dist(3) = 7). Memoised.

  var DIST_LIMIT = 4096;
  var DIST = [];
  for (var di = 0; di < DIST_LIMIT; di++) DIST.push(-1);
  DIST[1] = 0;
  var DIST_BIG = {};

  function distGet(v) {
    if (v < DIST_LIMIT) return DIST[v];
    var d = DIST_BIG[v];
    return d === undefined ? -1 : d;
  }
  function distSet(v, d) {
    if (v < DIST_LIMIT) DIST[v] = d; else DIST_BIG[v] = d;
  }

  function dist(v) {
    if (v <= 1) return 0;
    var d = distGet(v);
    if (d >= 0) return d;
    var path = [];
    var x = v;
    while (x > 1 && distGet(x) < 0) { path.push(x); x = f(x); }
    var base = x > 1 ? distGet(x) : 0;
    for (var i = path.length - 1; i >= 0; i--) {
      base++;
      distSet(path[i], base);
    }
    return base;
  }

  // ---------- doomed test ----------
  // Exact multiset test: can these values ever all be cleared by pairing
  // equal values? Merging a pair is always the required operation, so the
  // answer does not depend on order. Fixed point: while some value has two or
  // more tiles, replace a pair by f(v) (or drop both when v = 1).

  function isDoomed(cells) {
    var counts = {};
    var queue = [];
    var i, v;
    for (i = 0; i < cells.length; i++) {
      v = cells[i];
      if (v > 0) {
        counts[v] = (counts[v] || 0) + 1;
        if (counts[v] === 2) queue.push(v);
      }
    }
    while (queue.length) {
      v = queue.pop();
      while (counts[v] >= 2) {
        counts[v] -= 2;
        var w = f(v);
        if (w) {
          counts[w] = (counts[w] || 0) + 1;
          if (counts[w] === 2) queue.push(w);
        }
      }
    }
    for (var key in counts) {
      if (counts[key] > 0) return true;
    }
    return false;
  }

  // ---------- sliding ----------
  // Lines are processed from the swipe edge outward, a tile slides until
  // blocked, and merges into the tile ahead when values match and that tile
  // is not itself a merge product of this move. Ghost cells from clears block
  // like merge products and are emptied when the move ends.
  //
  // When `track` is given it must be an object; it receives
  //   sources: 16 entries, null or [i] or [ahead, mover] of input indices
  //   clears:  [{ at, a, b }] for every cleared pair (cell `at` ends empty)

  var FLAGS = [];
  for (var fi = 0; fi < CELLS; fi++) FLAGS.push(0);

  // src[off..off+15] -> dst (plain array of 16). Returns a packed int:
  // bit 0 moved, bits 1..5 merges (including clears), bits 6.. cleared pairs.
  function slide(src, off, dir, dst, track) {
    var i;
    var srcs = null;
    if (track) {
      srcs = [];
      for (i = 0; i < CELLS; i++) srcs.push(src[off + i] ? [i] : null);
      track.sources = srcs;
      track.clears = [];
    }
    for (i = 0; i < CELLS; i++) { dst[i] = src[off + i]; FLAGS[i] = 0; }
    var dr = dir === 0 ? -1 : dir === 2 ? 1 : 0;
    var dc = dir === 3 ? -1 : dir === 1 ? 1 : 0;
    var moved = 0, merges = 0, cleared = 0;

    for (var ri = 0; ri < SIZE; ri++) {
      var r = dr === 1 ? SIZE - 1 - ri : ri;
      for (var ci = 0; ci < SIZE; ci++) {
        var c = dc === 1 ? SIZE - 1 - ci : ci;
        var idx = r * SIZE + c;
        var v = dst[idx];
        if (!v || v === GHOST) continue;

        var nr = r, nc = c;
        while (true) {
          var tr = nr + dr, tc = nc + dc;
          if (tr < 0 || tr >= SIZE || tc < 0 || tc >= SIZE) break;
          if (dst[tr * SIZE + tc]) break;
          nr = tr; nc = tc;
        }

        var ar = nr + dr, ac = nc + dc;
        var aidx = (ar >= 0 && ar < SIZE && ac >= 0 && ac < SIZE) ? ar * SIZE + ac : -1;

        if (aidx >= 0 && dst[aidx] === v && !FLAGS[aidx]) {
          dst[idx] = 0;
          var m = f(v);
          FLAGS[aidx] = 1;
          if (m) {
            dst[aidx] = m;
          } else {
            dst[aidx] = GHOST;
            cleared++;
          }
          merges++;
          moved = 1;
          if (srcs) {
            var pair = [srcs[aidx][0], srcs[idx][0]];
            srcs[idx] = null;
            if (m) {
              srcs[aidx] = pair;
            } else {
              srcs[aidx] = null;
              track.clears.push({ at: aidx, a: pair[0], b: pair[1] });
            }
          }
        } else if (nr !== r || nc !== c) {
          dst[idx] = 0;
          dst[nr * SIZE + nc] = v;
          moved = 1;
          if (srcs) {
            srcs[nr * SIZE + nc] = srcs[idx];
            srcs[idx] = null;
          }
        }
      }
    }
    if (cleared) {
      for (i = 0; i < CELLS; i++) if (dst[i] === GHOST) dst[i] = 0;
    }
    return moved | (merges << 1) | (cleared << 6);
  }

  // Pure. Returns { cells, moved, merges, cleared } and, when track is true,
  // also sources and clears (see slide()).
  function applyMove(cells, dir, track) {
    var out = [];
    for (var i = 0; i < CELLS; i++) out.push(0);
    var t = track ? {} : null;
    var res = slide(cells, 0, dir, out, t);
    var result = {
      cells: out,
      moved: (res & 1) === 1,
      merges: (res >> 1) & 31,
      cleared: res >> 6
    };
    if (t) {
      result.sources = t.sources;
      result.clears = t.clears;
    }
    return result;
  }

  function isEmpty(cells) {
    for (var i = 0; i < CELLS; i++) if (cells[i]) return false;
    return true;
  }

  function anyMove(cells) {
    var buf = [];
    for (var i = 0; i < CELLS; i++) buf.push(0);
    for (var d = 0; d < 4; d++) {
      if (slide(cells, 0, d, buf, null) & 1) return true;
    }
    return false;
  }

  function countTiles(cells) {
    var n = 0;
    for (var i = 0; i < CELLS; i++) if (cells[i]) n++;
    return n;
  }

  // ---------- solver ----------
  // Breadth first search over boards. Moves that do not change the board are
  // skipped and boards are deduplicated by a 16 character key. nodes counts
  // distinct boards generated (root included); the cap applies to that count
  // and is only a guard: with no spawns the reachable space is a few hundred
  // to a couple of thousand boards.
  //
  // The multiset test is only applied to the root: no legal move can turn a
  // clearable multiset into a doomed one, so boards fail mid-game on geometry
  // alone and the search has to prove that by exhausting the frontier.

  function keyChars(cells) {
    return String.fromCharCode.apply(null, cells);
  }
  function keyJoin(cells) {
    return cells.join(',');
  }

  function solve(cells, maxNodes) {
    var cap = maxNodes > 0 ? maxNodes : DEFAULT_NODES;
    var root = [], i, maxv = 0;
    for (i = 0; i < CELLS; i++) {
      var v = cells[i] > 0 ? cells[i] : 0;
      root.push(v);
      if (v > maxv) maxv = v;
    }
    var keyOf = maxv < 65536 ? keyChars : keyJoin;

    if (isEmpty(root)) return { solved: true, moves: [], nodes: 1, exhausted: false };
    if (isDoomed(root)) return { solved: false, moves: null, nodes: 1, exhausted: false };

    var capacity = 4096;
    var pool = new Int32Array(capacity * CELLS);
    var parents = new Int32Array(capacity);
    var dirs = new Int8Array(capacity);
    var count = 0;
    var seen = Object.create(null);
    var buf = [];
    for (i = 0; i < CELLS; i++) { buf.push(0); pool[i] = root[i]; }
    parents[0] = -1; dirs[0] = -1; count = 1;
    seen[keyOf(root)] = 1;
    var nodes = 1;
    var head = 0;

    while (head < count) {
      var off = head * CELLS;
      for (var d = 0; d < 4; d++) {
        var res = slide(pool, off, d, buf, null);
        if (!(res & 1)) continue;
        var k = keyOf(buf);
        if (seen[k]) continue;
        seen[k] = 1;
        nodes++;
        if ((res >> 6) && isEmpty(buf)) {
          var moves = [d];
          for (var p = head; p > 0; p = parents[p]) moves.push(dirs[p]);
          moves.reverse();
          return { solved: true, moves: moves, nodes: nodes, exhausted: false };
        }
        if (count === capacity) {
          capacity *= 2;
          var np = new Int32Array(capacity * CELLS); np.set(pool); pool = np;
          var npar = new Int32Array(capacity); npar.set(parents); parents = npar;
          var nd = new Int8Array(capacity); nd.set(dirs); dirs = nd;
        }
        var base = count * CELLS;
        for (i = 0; i < CELLS; i++) pool[base + i] = buf[i];
        parents[count] = head;
        dirs[count] = d;
        count++;
        if (nodes >= cap) return { solved: false, moves: null, nodes: nodes, exhausted: true };
      }
      head++;
    }
    return { solved: false, moves: null, nodes: nodes, exhausted: false };
  }

  // ---------- level table ----------

  // tiles      board size at the start of the level
  // maxPairs   the reverse construction starts from 1 or 2 pairs of 1s
  // pOdd       when a value has an odd parent, probability of splitting that way
  // pDeep      probability that a split picks the tile farthest from 1 instead
  //            of a uniformly random one (builds long chains, so odd values
  //            and a higher par)
  // minOdd     boards must contain at least this many odd values
  // candidates boards generated per level; the one with the longest par wins
  // maxValue   ceiling on any value the construction may create
  function params(level) {
    var L = level > 1 ? Math.floor(level) : 1;
    var tiles = Math.min(5 + L, 12);
    return {
      level: L,
      tiles: tiles,
      maxPairs: tiles <= 8 ? 1 : 2,
      pOdd: 0.7,
      pDeep: L <= 4 ? 0.2 : L <= 7 ? 0.3 : Math.min(0.45, 0.3 + 0.03 * (L - 7)),
      minOdd: L >= 2 ? 1 : 0,
      candidates: L <= 4 ? 1 : 2,
      maxValue: L <= 6 ? 100 : 400,
      maxNodes: DEFAULT_NODES,
      placements: 10,
      maxRegens: 300
    };
  }

  // Level 1 is a fixed teaching board: the first swipe shows an odd pair
  // growing (5+5 -> 16), then a halving chain down to a vanishing pair of 1s.
  var LEVEL_ONE = [
    5, 5, 16, 8,
    0, 0, 0, 4,
    0, 0, 0, 2,
    0, 0, 0, 1
  ];

  // ---------- generator ----------

  // u odd, u >= 3 with 3u + 1 = v, or 0 when there is none.
  function oddParent(v) {
    if (v < 10 || (v - 1) % 3 !== 0) return 0;
    var u = (v - 1) / 3;
    return (u % 2 === 1) ? u : 0;
  }

  function countOdd(values) {
    var n = 0;
    for (var i = 0; i < values.length; i++) if (values[i] % 2 === 1 && values[i] > 1) n++;
    return n;
  }

  // Reverse construction: start from pairs of 1s and keep replacing one tile
  // v by two tiles u with f(u) = v until there are p.tiles tiles. Returns the
  // list of values, or null when every tile is at the value ceiling.
  function buildValues(p, rng) {
    var pairs = p.maxPairs > 1 && rng() < 0.5 ? 2 : 1;
    var vals = [];
    var i, v, even, odd;
    for (i = 0; i < pairs * 2; i++) vals.push(1);
    while (vals.length < p.tiles) {
      var cand = [];
      for (i = 0; i < vals.length; i++) {
        v = vals[i];
        even = 2 * v <= p.maxValue;
        odd = oddParent(v);
        if (odd > p.maxValue) odd = 0;
        if (even || odd) cand.push(i);
      }
      if (!cand.length) return null;
      var idx;
      if (p.pDeep > 0 && rng() < p.pDeep) {
        // farthest from 1 among the splittable tiles, ties broken at random
        var best = -1, ties = [];
        for (i = 0; i < cand.length; i++) {
          var dd = dist(vals[cand[i]]);
          if (dd > best) { best = dd; ties = [cand[i]]; }
          else if (dd === best) ties.push(cand[i]);
        }
        idx = ties[Math.floor(rng() * ties.length)];
      } else {
        idx = cand[Math.floor(rng() * cand.length)];
      }
      v = vals[idx];
      even = 2 * v <= p.maxValue;
      odd = oddParent(v);
      if (odd > p.maxValue) odd = 0;
      var u = (odd && (!even || rng() < p.pOdd)) ? odd : 2 * v;
      vals[idx] = u;
      vals.push(u);
    }
    return vals;
  }

  function place(values, rng) {
    var order = [];
    var i;
    for (i = 0; i < CELLS; i++) order.push(i);
    for (i = CELLS - 1; i > 0; i--) {
      var j = Math.floor(rng() * (i + 1));
      var t = order[i]; order[i] = order[j]; order[j] = t;
    }
    var cells = [];
    for (i = 0; i < CELLS; i++) cells.push(0);
    for (i = 0; i < values.length; i++) cells[order[i]] = values[i];
    return cells;
  }

  // Returns { cells, par, moves, values, attempts, nodes, peakNodes, regens }.
  // par is the exact shortest solution for the returned board. opts may
  // override any params field. With candidates > 1 the board with the
  // longest par is kept.
  function generate(level, rng, opts) {
    rng = rng || Math.random;
    var p = params(level);
    if (opts) {
      for (var key in opts) {
        if (Object.prototype.hasOwnProperty.call(opts, key)) p[key] = opts[key];
      }
    }
    if (p.level === 1 && !opts) {
      var fixed = LEVEL_ONE.slice();
      var sol = solve(fixed, p.maxNodes);
      return { cells: fixed, par: sol.moves.length, moves: sol.moves, values: fixed.filter(function (v) { return v > 0; }), attempts: 1, nodes: sol.nodes, peakNodes: sol.nodes, regens: 0 };
    }
    var attempts = 0, nodes = 0, peak = 0, regens = 0;
    var bestBoard = null;
    for (var c = 0; c < p.candidates; c++) {
      var board = null;
      while (!board) {
        // Relax the constraints rather than loop forever if a parameter set
        // turns out to be hard to satisfy.
        if (regens >= p.maxRegens) { p.minOdd = 0; p.pDeep = 0; }
        var values = buildValues(p, rng);
        if (values && countOdd(values) >= p.minOdd) {
          for (var a = 0; a < p.placements && !board; a++) {
            var cells = place(values, rng);
            var res = solve(cells, p.maxNodes);
            attempts++;
            nodes += res.nodes;
            if (res.nodes > peak) peak = res.nodes;
            if (res.solved) board = { cells: cells, par: res.moves.length, moves: res.moves, values: values };
          }
        }
        if (!board) regens++;
      }
      if (!bestBoard || board.par > bestBoard.par) bestBoard = board;
    }
    bestBoard.attempts = attempts;
    bestBoard.nodes = nodes;
    bestBoard.peakNodes = peak;
    bestBoard.regens = regens;
    return bestBoard;
  }

  var Collatz = {
    f: f,
    dist: dist,
    isDoomed: isDoomed,
    applyMove: applyMove,
    anyMove: anyMove,
    isEmpty: isEmpty,
    countTiles: countTiles,
    solve: solve,
    params: params,
    generate: generate,
    SIZE: SIZE,
    LEVEL_ONE: LEVEL_ONE,
    // internals exposed for tests
    _buildValues: buildValues,
    _place: place
  };

  if (typeof module !== 'undefined' && module.exports) module.exports = Collatz;
  if (typeof window !== 'undefined') window.Collatz = Collatz;
})();
