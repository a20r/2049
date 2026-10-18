/*
 * Tests for collatz.js. Run: node test/collatz.test.js
 * Prints generator statistics per level after the assertions.
 */
'use strict';

var assert = require('assert');
var path = require('path');
var Collatz = require(path.join(__dirname, '..', 'collatz.js'));

function mulberry32(seed) {
  var a = seed >>> 0;
  return function () {
    a = (a + 0x6D2B79F5) >>> 0;
    var t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function row(cells, r) { return cells.slice(r * 4, r * 4 + 4); }

// ---------- rule ----------
assert.strictEqual(Collatz.f(1), 0);
assert.strictEqual(Collatz.f(2), 1);
assert.strictEqual(Collatz.f(3), 10);
assert.strictEqual(Collatz.f(10), 5);
assert.strictEqual(Collatz.f(16), 8);
assert.strictEqual(Collatz.dist(3), 7);
assert.strictEqual(Collatz.dist(27), 111);

// ---------- ghost semantics ----------
var r1 = Collatz.applyMove([1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 3, true);
assert.deepStrictEqual(row(r1.cells, 0), [0, 1, 0, 0], 'third 1 stops behind the cleared cell');
assert.strictEqual(r1.cleared, 1);
assert.deepStrictEqual(r1.clears, [{ at: 0, a: 0, b: 1 }]);
assert.deepStrictEqual(r1.sources[1], [2]);

var r2 = Collatz.applyMove([2, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 3, true);
assert.deepStrictEqual(row(r2.cells, 0), [2, 0, 2, 0], 'trailing 2 does not slide through a cleared pair');
assert.deepStrictEqual(r2.sources[2], [3]);

var r3 = Collatz.applyMove([5, 5, 16, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 3, true);
assert.deepStrictEqual(row(r3.cells, 0), [16, 16, 8, 0]);
assert.deepStrictEqual(r3.sources[0], [0, 1], 'ahead tile first, mover second');
assert.strictEqual(r3.merges, 1);

var r4 = Collatz.applyMove([4, 4, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 1, true);
assert.deepStrictEqual(row(r4.cells, 0), [0, 0, 2, 2], 'merge products do not merge again this move');

var r5 = Collatz.applyMove([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, true);
assert.strictEqual(r5.moved, false);

// A tracked and an untracked move must agree on every random board.
var rng = mulberry32(11);
var VALUES = [1, 2, 3, 4, 5, 6, 8, 10, 16, 20];
for (var t = 0; t < 2000; t++) {
  var board = [];
  for (var i = 0; i < 16; i++) board.push(rng() < 0.55 ? VALUES[Math.floor(rng() * VALUES.length)] : 0);
  for (var d = 0; d < 4; d++) {
    var a = Collatz.applyMove(board, d);
    var b = Collatz.applyMove(board, d, true);
    assert.deepStrictEqual(a.cells, b.cells);
    assert.strictEqual(a.moved, b.moved);
    // every output tile has sources, every source maps to an input tile of the same count
    var seen = {};
    for (i = 0; i < 16; i++) {
      if (b.cells[i]) {
        assert.ok(b.sources[i] && b.sources[i].length >= 1, 'tile without sources');
        b.sources[i].forEach(function (s) { assert.ok(board[s] > 0); assert.ok(!seen[s]); seen[s] = 1; });
        if (b.sources[i].length === 2) {
          assert.strictEqual(board[b.sources[i][0]], board[b.sources[i][1]]);
          assert.strictEqual(b.cells[i], Collatz.f(board[b.sources[i][0]]));
        } else {
          assert.strictEqual(b.cells[i], board[b.sources[i][0]]);
        }
      } else {
        assert.strictEqual(b.sources[i], null);
      }
    }
    b.clears.forEach(function (c) {
      assert.strictEqual(board[c.a], 1); assert.strictEqual(board[c.b], 1);
      assert.ok(!seen[c.a] && !seen[c.b]); seen[c.a] = seen[c.b] = 1;
      assert.strictEqual(b.cells[c.at], 0);
    });
    var inputs = 0;
    for (i = 0; i < 16; i++) if (board[i]) inputs++;
    assert.strictEqual(Object.keys(seen).length, inputs, 'every input tile accounted for');
  }
}

// ---------- doomed ----------
function bruteClearable(values) {
  var memo = {};
  function rec(ms) {
    if (!ms.length) return true;
    var key = ms.join(',');
    if (memo[key] !== undefined) return memo[key];
    var ok = false, tried = {};
    for (var i = 0; i < ms.length && !ok; i++) {
      var v = ms[i];
      if (tried[v]) continue;
      tried[v] = true;
      var j = ms.indexOf(v, i + 1);
      if (j < 0) continue;
      var next = ms.slice();
      next.splice(j, 1); next.splice(i, 1);
      var w = Collatz.f(v);
      if (w) next.push(w);
      next.sort(function (x, y) { return x - y; });
      ok = rec(next);
    }
    memo[key] = ok;
    return ok;
  }
  return rec(values.slice().sort(function (x, y) { return x - y; }));
}
assert.strictEqual(Collatz.isDoomed([4, 4, 2, 2]), true);
assert.strictEqual(Collatz.isDoomed([3, 3, 10, 5, 5, 16]), true);
assert.strictEqual(Collatz.isDoomed([3, 3, 10, 5, 16, 8, 4, 2, 1]), false, 'magnitude order would wrongly say doomed');
assert.strictEqual(Collatz.isDoomed([16, 16]), true);
assert.strictEqual(Collatz.isDoomed([]), false);
rng = mulberry32(5);
for (t = 0; t < 3000; t++) {
  var n = 2 + Math.floor(rng() * 9);
  var ms = [];
  for (i = 0; i < n; i++) ms.push(VALUES[Math.floor(rng() * VALUES.length)]);
  assert.strictEqual(Collatz.isDoomed(ms), !bruteClearable(ms), 'doomed mismatch on ' + ms.join(','));
}

// ---------- solver ----------
var one = Collatz.solve(Collatz.LEVEL_ONE);
assert.ok(one.solved, 'level one is solvable');
var replay = Collatz.LEVEL_ONE.slice();
one.moves.forEach(function (dir) { replay = Collatz.applyMove(replay, dir).cells; });
assert.ok(Collatz.isEmpty(replay), 'solution replays to an empty board');
assert.strictEqual(Collatz.solve([]).solved, true);
var dead = Collatz.solve([2, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
assert.strictEqual(dead.solved, false);
assert.strictEqual(dead.exhausted, false, 'dead end is proven, not capped');

// ---------- generator ----------
rng = mulberry32(99);
var g1 = Collatz.generate(1, rng);
assert.deepStrictEqual(g1.cells, Collatz.LEVEL_ONE);
assert.strictEqual(g1.par, one.moves.length);

// ---------- daily ----------
assert.strictEqual(Collatz.dayNumber('2026-10-18'), 1);
assert.strictEqual(Collatz.dayNumber('2026-10-19'), 2);
assert.strictEqual(Collatz.dayNumber('2027-01-01'), 76);
assert.ok(/^\d{4}-\d{2}-\d{2}$/.test(Collatz.dateKey()));
assert.strictEqual(Collatz.dateKey(new Date(2026, 0, 5)), '2026-01-05');
var d1 = Collatz.daily('2026-10-18'), d2 = Collatz.daily('2026-10-18'), d3 = Collatz.daily('2026-10-19');
assert.deepStrictEqual(d1.cells, d2.cells, 'same date gives the same board');
assert.notDeepStrictEqual(d1.cells, d3.cells, 'different dates give different boards');
assert.strictEqual(d1.day, 1);
assert.strictEqual(Collatz.countTiles(d1.cells), 12);
assert.ok(d1.par > 0 && Collatz.solve(d1.cells).moves.length === d1.par, 'daily par is exact');
var dailyPars = [];
for (var dd = 1; dd <= 31; dd++) dailyPars.push(Collatz.daily('2026-12-' + (dd < 10 ? '0' : '') + dd).par);
console.log('daily par, Dec 2026: min ' + Math.min.apply(null, dailyPars) + ' mean ' + (dailyPars.reduce(function (x, y) { return x + y; }, 0) / 31).toFixed(1) + ' max ' + Math.max.apply(null, dailyPars));

console.log('assertions passed. level one par = ' + one.moves.length + ', moves = ' + one.moves.join(''));
console.log('');
console.log('level | tiles | boards | par min/mean/max | odd tiles % | boards with odd % | max value | regens mean | ms mean | ms max');
var LEVELS = [2, 3, 4, 5, 6, 7, 8, 10, 12, 15, 20];
LEVELS.forEach(function (L) {
  var N = 120;
  var pars = [], odd = 0, tiles = 0, withOdd = 0, maxv = 0, regens = 0, ms = [];
  var lrng = mulberry32(1000 + L);
  for (var k = 0; k < N; k++) {
    var t0 = process.hrtime();
    var g = Collatz.generate(L, lrng);
    var dt = process.hrtime(t0);
    ms.push(dt[0] * 1e3 + dt[1] / 1e6);
    pars.push(g.par);
    regens += g.regens;
    var hasOdd = false;
    g.cells.forEach(function (v) {
      if (!v) return;
      tiles++;
      if (v > maxv) maxv = v;
      if (v % 2 === 1 && v > 1) { odd++; hasOdd = true; }
    });
    if (hasOdd) withOdd++;
    // the returned par must be exact for the returned board under the game's slide rule
    var rep = g.cells.slice();
    g.moves.forEach(function (dir) { rep = Collatz.applyMove(rep, dir).cells; });
    assert.ok(Collatz.isEmpty(rep));
    assert.strictEqual(Collatz.countTiles(g.cells), Collatz.params(L).tiles);
    assert.ok(!Collatz.isDoomed(g.cells));
  }
  var sum = pars.reduce(function (x, y) { return x + y; }, 0);
  var msSum = ms.reduce(function (x, y) { return x + y; }, 0);
  console.log([L, Collatz.params(L).tiles, N,
    Math.min.apply(null, pars) + '/' + (sum / N).toFixed(1) + '/' + Math.max.apply(null, pars),
    (100 * odd / tiles).toFixed(1), (100 * withOdd / N).toFixed(0), maxv, (regens / N).toFixed(1),
    (msSum / N).toFixed(2), Math.max.apply(null, ms).toFixed(1)].join(' | '));
});
