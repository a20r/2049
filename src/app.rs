//! The game: four modes over one board, rendered straight into the DOM.
//!
//! 2048 is the classic rule. Collatz, Daily and Endless share the Collatz
//! merge rule (n/2, 3n+1, two 1s vanish) and the parity tile styling; the
//! first two are spawn-free puzzles with an exact solver behind them, the
//! last spawns a tile every move and is about keeping the board clear.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use nexus_engine::collatz::{self, Cells, MoveResult, Source, CELLS, SIZE};
use nexus_engine::{classic, daily};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AddEventListenerOptions, Element, HtmlButtonElement, HtmlElement, KeyboardEvent, MouseEvent,
    TouchEvent,
};

use crate::dom;
use crate::storage::*;

const WIN_VALUE: u32 = 2048;
/// Slightly longer than the CSS slide so the swap lands after it.
const MOVE_MS: i32 = 150;
const UNDO_CAP: usize = 100;
const SWIPE_MIN: f64 = 24.0;
const LOSS_LINES: [&str; 4] = [
    "ALL THOSE MOMENTS WILL BE LOST IN TIME",
    "LIKE TEARS IN RAIN",
    "RETIREMENT COMPLETE",
    "YOUR INCEPT DATE HAS EXPIRED",
];
const MIDDOT: &str = " \u{b7} ";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    Classic,
    Collatz,
    Daily,
    Endless,
}

impl Mode {
    fn key(self) -> &'static str {
        match self {
            Mode::Classic => "2048",
            Mode::Collatz => "collatz",
            Mode::Daily => "daily",
            Mode::Endless => "endless",
        }
    }
    fn from_key(s: &str) -> Mode {
        match s {
            "collatz" => Mode::Collatz,
            "daily" => Mode::Daily,
            "endless" => Mode::Endless,
            _ => Mode::Classic,
        }
    }
    /// Collatz rules and parity tile styling.
    fn is_cz(self) -> bool {
        !matches!(self, Mode::Classic)
    }
    /// Spawn-free puzzle with a solver, par and a full undo stack.
    fn is_puzzle(self) -> bool {
        matches!(self, Mode::Collatz | Mode::Daily)
    }
}

#[derive(Clone, Copy, Debug)]
struct Tile {
    id: u32,
    value: u32,
    row: usize,
    col: usize,
}

type Grid = [[Option<Tile>; SIZE]; SIZE];

/// 2048's single-step undo keeps tile ids so the restore animates nothing.
#[derive(Clone)]
struct Snapshot {
    cells: Vec<Option<(u32, u32)>>,
    score: u32,
    won: bool,
    keep_playing: bool,
}

#[derive(Clone)]
struct CzState {
    level: u32,
    date: Option<String>,
    moves: u32,
    par: u32,
    start: Cells,
    undo: Vec<(Cells, u32)>,
    undos: u32,
    stuck: bool,
    cleared: bool,
}

#[derive(Clone, Default)]
struct EndlessState {
    score: u32,
    over: bool,
    history: Option<(Cells, u32)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    NewGame,
    KeepGoing,
    UndoCz,
    Retry,
    NewBoard,
    NextLevel,
    Share,
    Begin,
    NewRun,
}

struct OverlayButton {
    label: &'static str,
    action: Action,
}

#[derive(Default)]
struct OverlaySpec {
    win: bool,
    kicker: String,
    title: String,
    sub: String,
    wide: bool,
    primary: Option<OverlayButton>,
    secondary: Option<OverlayButton>,
    alt: Option<OverlayButton>,
    announce: String,
}

struct Merge {
    tile: Tile,
    sources: [Tile; 2],
}

struct ClearAnim {
    a: Tile,
    b: Tile,
}

struct Pending {
    merges: Vec<Merge>,
    clears: Vec<ClearAnim>,
}

/// End-of-move work that runs when the slide timer fires or is flushed.
type Finish = Box<dyn FnOnce(&mut App)>;

/// Visual theme: the Blade Runner neon look, or a modern take on the original 2048.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Neon,
    Classic,
}

impl Theme {
    fn key(self) -> &'static str {
        match self {
            Theme::Neon => "neon",
            Theme::Classic => "classic",
        }
    }
    fn from_key(s: &str) -> Theme {
        if s == "classic" {
            Theme::Classic
        } else {
            Theme::Neon
        }
    }
    /// Browser chrome colour (the theme-color meta) for each theme.
    fn chrome(self) -> &'static str {
        match self {
            Theme::Neon => "#07020f",
            Theme::Classic => "#faf8ef",
        }
    }
    fn other(self) -> Theme {
        match self {
            Theme::Neon => Theme::Classic,
            Theme::Classic => Theme::Neon,
        }
    }
}

const THEMES: [Theme; 2] = [Theme::Neon, Theme::Classic];

struct Els {
    board: HtmlElement,
    tiles: HtmlElement,
    score: HtmlElement,
    best: HtmlElement,
    score_label: HtmlElement,
    best_label: HtmlElement,
    delta: HtmlElement,
    status: HtmlElement,
    status_line: HtmlElement,
    tagline: HtmlElement,
    undo: HtmlButtonElement,
    new_game: HtmlButtonElement,
    mode_btns: [HtmlButtonElement; 4],
    theme_btns: [HtmlButtonElement; 2],
    overlay: HtmlElement,
    kicker: HtmlElement,
    title: HtmlElement,
    sub: HtmlElement,
    primary: HtmlButtonElement,
    secondary: HtmlButtonElement,
    alt: HtmlButtonElement,
}

pub struct App {
    me: Weak<RefCell<App>>,
    els: Els,
    mode: Mode,
    theme: Theme,
    next_id: u32,
    grid: Grid,
    elements: HashMap<u32, HtmlElement>,
    animating: bool,
    queued_dir: Option<u8>,
    move_timer: Option<i32>,
    pending_finish: Option<Finish>,
    timer_cb: Option<Closure<dyn FnMut()>>,
    intro_open: bool,
    actions: [Option<Action>; 3],
    // In-memory saves per mode, so a mode switch is lossless even without storage.
    mem_classic: Option<(ClassicSave, Option<Snapshot>)>,
    mem_cz: HashMap<Mode, CzSave>,
    mem_endless: Option<EndlessSave>,
    // 2048
    score: u32,
    best: u32,
    won: bool,
    keep_playing: bool,
    over: bool,
    history: Option<Snapshot>,
    // Collatz and Daily
    cz: Option<CzState>,
    cz_best: u32,
    // Endless
    en: EndlessState,
    en_best: u32,
    // touch
    touch_x: f64,
    touch_y: f64,
    touching: bool,
}

const MODES: [Mode; 4] = [Mode::Classic, Mode::Collatz, Mode::Daily, Mode::Endless];

pub fn boot() {
    let els = Els {
        board: dom::el("board"),
        tiles: dom::el("tiles"),
        score: dom::el("score"),
        best: dom::el("best"),
        score_label: dom::el("score-label"),
        best_label: dom::el("best-label"),
        delta: dom::el("score-delta"),
        status: dom::el("status"),
        status_line: dom::el("status-line"),
        tagline: dom::el("tagline"),
        undo: dom::btn("undo"),
        new_game: dom::btn("new-game"),
        mode_btns: [
            dom::btn("mode-2048"),
            dom::btn("mode-collatz"),
            dom::btn("mode-daily"),
            dom::btn("mode-endless"),
        ],
        theme_btns: [dom::btn("theme-neon"), dom::btn("theme-classic")],
        overlay: dom::el("overlay"),
        kicker: dom::el("overlay-kicker"),
        title: dom::el("overlay-title"),
        sub: dom::el("overlay-sub"),
        primary: dom::btn("overlay-primary"),
        secondary: dom::btn("overlay-secondary"),
        alt: dom::btn("overlay-alt"),
    };
    let app = Rc::new(RefCell::new(App {
        me: Weak::new(),
        els,
        mode: Mode::Classic,
        theme: Theme::Neon,
        next_id: 1,
        grid: [[None; SIZE]; SIZE],
        elements: HashMap::new(),
        animating: false,
        queued_dir: None,
        move_timer: None,
        pending_finish: None,
        timer_cb: None,
        intro_open: false,
        actions: [None; 3],
        mem_classic: None,
        mem_cz: HashMap::new(),
        mem_endless: None,
        score: 0,
        best: 0,
        won: false,
        keep_playing: false,
        over: false,
        history: None,
        cz: None,
        cz_best: 0,
        en: EndlessState::default(),
        en_best: 0,
        touch_x: 0.0,
        touch_y: 0.0,
        touching: false,
    }));
    {
        let mut a = app.borrow_mut();
        a.me = Rc::downgrade(&app);
        let weak = Rc::downgrade(&app);
        a.timer_cb = Some(Closure::wrap(Box::new(move || {
            if let Some(app) = weak.upgrade() {
                app.borrow_mut().run_pending();
            }
        }) as Box<dyn FnMut()>));
    }
    wire_events(&app);
    let mut a = app.borrow_mut();
    a.best = dom::local_get(CLASSIC_BEST_KEY)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    a.cz_best = dom::local_get(CZ_BEST_KEY)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    a.en_best = dom::local_get(ENDLESS_BEST_KEY)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let theme = dom::local_get(THEME_KEY).unwrap_or_default();
    a.set_theme(Theme::from_key(&theme), true);
    let saved = dom::local_get(MODE_KEY).unwrap_or_default();
    a.set_mode(Mode::from_key(&saved), true);
    // The app lives for the page: keep the Rc alive.
    std::mem::forget(app.clone());
}

fn with_app<F: FnOnce(&mut App)>(weak: &Weak<RefCell<App>>, f: F) {
    if let Some(app) = weak.upgrade() {
        f(&mut app.borrow_mut());
    }
}

fn target_is_button(e: &web_sys::Event) -> bool {
    e.target()
        .and_then(|t| t.dyn_into::<Element>().ok())
        .and_then(|el| el.closest("button").ok().flatten())
        .is_some()
}

fn wire_events(app: &Rc<RefCell<App>>) {
    let doc = dom::document();
    let (board, undo, new_game, mode_btns, theme_btns, overlay_btns) = {
        let a = app.borrow();
        (
            a.els.board.clone(),
            a.els.undo.clone(),
            a.els.new_game.clone(),
            a.els.mode_btns.clone(),
            a.els.theme_btns.clone(),
            [
                a.els.primary.clone(),
                a.els.secondary.clone(),
                a.els.alt.clone(),
            ],
        )
    };

    let weak = Rc::downgrade(app);
    let on_key = Closure::wrap(Box::new(move |e: KeyboardEvent| {
        with_app(&weak, |a| a.on_key(&e));
    }) as Box<dyn FnMut(KeyboardEvent)>);
    let _ = doc.add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref());
    on_key.forget();

    let weak = Rc::downgrade(app);
    let on_touch_start = Closure::wrap(Box::new(move |e: TouchEvent| {
        if e.touches().length() != 1 || target_is_button(&e) {
            return;
        }
        if let Some(t) = e.touches().get(0) {
            with_app(&weak, |a| {
                a.on_start(t.client_x() as f64, t.client_y() as f64)
            });
        }
    }) as Box<dyn FnMut(TouchEvent)>);
    let passive = AddEventListenerOptions::new();
    passive.set_passive(true);
    let _ = board.add_event_listener_with_callback_and_add_event_listener_options(
        "touchstart",
        on_touch_start.as_ref().unchecked_ref(),
        &passive,
    );
    on_touch_start.forget();

    let weak = Rc::downgrade(app);
    let on_touch_move = Closure::wrap(Box::new(move |e: TouchEvent| {
        with_app(&weak, |a| {
            if !a.touching {
                return;
            }
            e.prevent_default();
            // Fire as soon as the swipe is unambiguous so the slide starts under the finger.
            if e.touches().length() == 1 {
                if let Some(t) = e.touches().get(0) {
                    a.on_move(t.client_x() as f64, t.client_y() as f64);
                }
            }
        });
    }) as Box<dyn FnMut(TouchEvent)>);
    let active = AddEventListenerOptions::new();
    active.set_passive(false);
    let _ = board.add_event_listener_with_callback_and_add_event_listener_options(
        "touchmove",
        on_touch_move.as_ref().unchecked_ref(),
        &active,
    );
    on_touch_move.forget();

    let weak = Rc::downgrade(app);
    let on_touch_end = Closure::wrap(Box::new(move |e: TouchEvent| {
        if let Some(t) = e.changed_touches().get(0) {
            with_app(&weak, |a| {
                a.on_end(t.client_x() as f64, t.client_y() as f64)
            });
        }
    }) as Box<dyn FnMut(TouchEvent)>);
    let _ = board.add_event_listener_with_callback_and_add_event_listener_options(
        "touchend",
        on_touch_end.as_ref().unchecked_ref(),
        &passive,
    );
    on_touch_end.forget();

    let weak = Rc::downgrade(app);
    let on_touch_cancel = Closure::wrap(Box::new(move |_e: TouchEvent| {
        with_app(&weak, |a| a.touching = false);
    }) as Box<dyn FnMut(TouchEvent)>);
    let _ = board.add_event_listener_with_callback_and_add_event_listener_options(
        "touchcancel",
        on_touch_cancel.as_ref().unchecked_ref(),
        &passive,
    );
    on_touch_cancel.forget();

    let weak = Rc::downgrade(app);
    let on_mouse_down = Closure::wrap(Box::new(move |e: MouseEvent| {
        if e.button() != 0 || target_is_button(&e) {
            return;
        }
        with_app(&weak, |a| {
            a.on_start(e.client_x() as f64, e.client_y() as f64)
        });
    }) as Box<dyn FnMut(MouseEvent)>);
    let _ =
        board.add_event_listener_with_callback("mousedown", on_mouse_down.as_ref().unchecked_ref());
    on_mouse_down.forget();

    let weak = Rc::downgrade(app);
    let on_mouse_up = Closure::wrap(Box::new(move |e: MouseEvent| {
        with_app(&weak, |a| {
            if a.touching {
                a.on_end(e.client_x() as f64, e.client_y() as f64);
            }
        });
    }) as Box<dyn FnMut(MouseEvent)>);
    let _ = doc.add_event_listener_with_callback("mouseup", on_mouse_up.as_ref().unchecked_ref());
    on_mouse_up.forget();

    let weak = Rc::downgrade(app);
    let on_undo =
        Closure::wrap(Box::new(move || with_app(&weak, |a| a.undo())) as Box<dyn FnMut()>);
    let _ = undo.add_event_listener_with_callback("click", on_undo.as_ref().unchecked_ref());
    on_undo.forget();

    let weak = Rc::downgrade(app);
    let on_new = Closure::wrap(
        Box::new(move || with_app(&weak, |a| a.primary_action())) as Box<dyn FnMut()>
    );
    let _ = new_game.add_event_listener_with_callback("click", on_new.as_ref().unchecked_ref());
    on_new.forget();

    for (i, b) in mode_btns.iter().enumerate() {
        let weak = Rc::downgrade(app);
        let mode = MODES[i];
        let cb = Closure::wrap(
            Box::new(move || with_app(&weak, |a| a.set_mode(mode, false))) as Box<dyn FnMut()>,
        );
        let _ = b.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    for (i, b) in theme_btns.iter().enumerate() {
        let weak = Rc::downgrade(app);
        let theme = THEMES[i];
        let cb = Closure::wrap(
            Box::new(move || with_app(&weak, |a| a.set_theme(theme, false))) as Box<dyn FnMut()>,
        );
        let _ = b.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
        cb.forget();
    }

    for (slot, b) in overlay_btns.iter().enumerate() {
        let weak = Rc::downgrade(app);
        let cb = Closure::wrap(
            Box::new(move || with_app(&weak, |a| a.overlay_click(slot))) as Box<dyn FnMut()>
        );
        let _ = b.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
        cb.forget();
    }
}

impl App {
    // ---------- grid helpers ----------

    fn make_tile(&mut self, value: u32, row: usize, col: usize) -> Tile {
        let id = self.next_id;
        self.next_id += 1;
        Tile {
            id,
            value,
            row,
            col,
        }
    }

    fn cells(&self) -> Cells {
        let mut cells = [0u32; CELLS];
        for r in 0..SIZE {
            for c in 0..SIZE {
                cells[r * SIZE + c] = self.grid[r][c].map(|t| t.value).unwrap_or(0);
            }
        }
        cells
    }

    fn grid_from(&mut self, cells: &Cells) -> Grid {
        let mut g: Grid = [[None; SIZE]; SIZE];
        for (i, v) in cells.iter().enumerate() {
            if *v > 0 {
                g[i / SIZE][i % SIZE] = Some(self.make_tile(*v, i / SIZE, i % SIZE));
            }
        }
        g
    }

    fn count_tiles(&self) -> u32 {
        collatz::count_tiles(&self.cells())
    }

    // ---------- 2048 state ----------

    fn classic_save(&self) -> ClassicSave {
        ClassicSave {
            cells: self.cells().to_vec(),
            score: self.score,
            won: self.won,
            keep_playing: self.keep_playing,
            over: self.over,
        }
    }

    fn snapshot(&self) -> Snapshot {
        let mut cells = Vec::with_capacity(CELLS);
        for r in 0..SIZE {
            for c in 0..SIZE {
                cells.push(self.grid[r][c].map(|t| (t.id, t.value)));
            }
        }
        Snapshot {
            cells,
            score: self.score,
            won: self.won,
            keep_playing: self.keep_playing,
        }
    }

    fn restore(&mut self, snap: &Snapshot) {
        self.grid = [[None; SIZE]; SIZE];
        for (i, s) in snap.cells.iter().enumerate() {
            if let Some((id, value)) = s {
                self.grid[i / SIZE][i % SIZE] = Some(Tile {
                    id: *id,
                    value: *value,
                    row: i / SIZE,
                    col: i % SIZE,
                });
            }
        }
        self.score = snap.score;
        self.won = snap.won;
        self.keep_playing = snap.keep_playing;
        self.over = false;
    }

    fn save_classic(&mut self) {
        if self.over {
            self.mem_classic = None;
            dom::local_remove(CLASSIC_KEY);
        } else {
            let s = self.classic_save();
            dom::local_set(CLASSIC_KEY, &to_json(&s));
            self.mem_classic = Some((s, self.history.clone()));
        }
    }

    fn apply_classic(&mut self, s: &ClassicSave) -> bool {
        let Some(cells) = cells_from(&s.cells) else {
            return false;
        };
        self.grid = self.grid_from(&cells);
        self.score = s.score;
        self.won = s.won;
        self.keep_playing = s.keep_playing;
        self.over = false;
        true
    }

    fn load_classic(&mut self) -> bool {
        if let Some((s, h)) = self.mem_classic.clone() {
            if self.apply_classic(&s) {
                self.history = h;
                return true;
            }
        }
        match parse::<ClassicSave>(dom::local_get(CLASSIC_KEY)) {
            Some(s) if self.apply_classic(&s) => {
                self.history = None;
                true
            }
            _ => false,
        }
    }

    // ---------- Collatz state ----------

    fn cz_key(&self) -> &'static str {
        if self.mode == Mode::Daily {
            DAILY_KEY
        } else {
            CZ_KEY
        }
    }

    fn cz_save(&self) -> CzSave {
        let cz = self.cz.as_ref().expect("cz state");
        CzSave {
            v: 1,
            level: cz.level,
            date: cz.date.clone(),
            moves: cz.moves,
            par: cz.par,
            start: cz.start.to_vec(),
            cells: self.cells().to_vec(),
            undo: cz
                .undo
                .iter()
                .map(|(c, m)| CzUndo {
                    cells: c.to_vec(),
                    moves: *m,
                })
                .collect(),
            undos: cz.undos,
            stuck: cz.stuck,
            cleared: cz.cleared,
        }
    }

    fn save_cz(&mut self) {
        let s = self.cz_save();
        dom::local_set(self.cz_key(), &to_json(&s));
        self.mem_cz.insert(self.mode, s);
    }

    fn apply_cz(&mut self, s: &CzSave) -> bool {
        if s.v != 1 || s.level < 1 {
            return false;
        }
        let (Some(cells), Some(start)) = (cells_from(&s.cells), cells_from(&s.start)) else {
            return false;
        };
        // A daily save only counts for the day it was made.
        if self.mode == Mode::Daily && s.date.as_deref() != Some(dom::today_key().as_str()) {
            return false;
        }
        let mut undo: Vec<(Cells, u32)> = s
            .undo
            .iter()
            .filter_map(|u| cells_from(&u.cells).map(|c| (c, u.moves)))
            .collect();
        if undo.len() > UNDO_CAP {
            undo.drain(0..undo.len() - UNDO_CAP);
        }
        self.cz = Some(CzState {
            level: s.level,
            date: s.date.clone(),
            moves: s.moves,
            par: s.par,
            start,
            undo,
            undos: s.undos,
            stuck: s.stuck,
            cleared: s.cleared,
        });
        self.grid = self.grid_from(&cells);
        true
    }

    fn load_cz(&mut self) -> bool {
        if let Some(s) = self.mem_cz.get(&self.mode).cloned() {
            if self.apply_cz(&s) {
                return true;
            }
        }
        match parse::<CzSave>(dom::local_get(self.cz_key())) {
            Some(s) => self.apply_cz(&s),
            None => false,
        }
    }

    fn history_map() -> History {
        parse::<History>(dom::local_get(DAILY_HIST_KEY)).unwrap_or_default()
    }

    /// Consecutive cleared days ending on `key`.
    fn streak_ending(hist: &History, key: &str) -> u32 {
        let mut n = 0;
        let mut k = key.to_string();
        while hist.contains_key(&k) && n < 10000 {
            n += 1;
            k = daily::shift_date_key(&k, -1);
        }
        n
    }

    // ---------- Endless state ----------

    fn save_endless(&mut self) {
        if self.en.over {
            self.mem_endless = None;
            dom::local_remove(ENDLESS_KEY);
        } else {
            let s = EndlessSave {
                cells: self.cells().to_vec(),
                score: self.en.score,
                over: false,
            };
            dom::local_set(ENDLESS_KEY, &to_json(&s));
            self.mem_endless = Some(s);
        }
    }

    fn load_endless(&mut self) -> bool {
        let s = self
            .mem_endless
            .clone()
            .or_else(|| parse::<EndlessSave>(dom::local_get(ENDLESS_KEY)));
        let Some(s) = s else { return false };
        if s.over {
            return false;
        }
        let Some(cells) = cells_from(&s.cells) else {
            return false;
        };
        self.grid = self.grid_from(&cells);
        self.en = EndlessState {
            score: s.score,
            over: false,
            history: None,
        };
        true
    }

    fn save_state(&mut self) {
        match self.mode {
            Mode::Classic => self.save_classic(),
            Mode::Collatz | Mode::Daily => self.save_cz(),
            Mode::Endless => self.save_endless(),
        }
    }

    // ---------- rendering ----------

    fn band_for(value: u32) -> u8 {
        match value {
            v if v < 4 => 0,
            v if v < 16 => 1,
            v if v < 64 => 2,
            _ => 3,
        }
    }

    fn tile_el(&mut self, tile: Tile) -> HtmlElement {
        let el = dom::create("div", "tile");
        dom::set_var(&el, "--x", &tile.col.to_string());
        dom::set_var(&el, "--y", &tile.row.to_string());
        let v = dom::create("span", "v");
        v.set_text_content(Some(&tile.value.to_string()));
        let _ = el.append_child(&v);
        if self.mode.is_cz() {
            let par = if tile.value == 1 {
                "one"
            } else if tile.value % 2 == 1 {
                "odd"
            } else {
                "even"
            };
            let _ = el.set_attribute("data-par", par);
            let _ = el.set_attribute("data-band", &Self::band_for(tile.value).to_string());
            let _ = el.set_attribute("data-len", &tile.value.to_string().len().to_string());
            let cap = dom::create("span", "cap");
            let text = if tile.value == 1 {
                "CLEAR".to_string()
            } else {
                format!("\u{2192}{}", collatz::f(tile.value))
            };
            cap.set_text_content(Some(&text));
            let _ = el.append_child(&cap);
        } else {
            let _ = el.set_attribute("data-v", &tile.value.to_string());
        }
        let _ = self.els.tiles.append_child(&el);
        self.elements.insert(tile.id, el.clone());
        el
    }

    fn position_el(&self, tile: &Tile) {
        if let Some(el) = self.elements.get(&tile.id) {
            dom::set_var(el, "--x", &tile.col.to_string());
            dom::set_var(el, "--y", &tile.row.to_string());
        }
    }

    fn remove_el(&mut self, id: u32) {
        if let Some(el) = self.elements.remove(&id) {
            el.remove();
        }
    }

    fn render_all(&mut self) {
        self.els.tiles.set_inner_html("");
        self.elements.clear();
        for r in 0..SIZE {
            for c in 0..SIZE {
                if let Some(t) = self.grid[r][c] {
                    self.tile_el(t);
                }
            }
        }
        self.render_score();
    }

    fn render_score(&self) {
        let (a, b) = match self.mode {
            Mode::Classic => (self.score.to_string(), self.best.to_string()),
            Mode::Collatz => match &self.cz {
                Some(cz) => (cz.level.to_string(), cz.moves.to_string()),
                None => ("1".into(), "0".into()),
            },
            Mode::Daily => match &self.cz {
                Some(cz) => (format!("#{}", cz.level), cz.moves.to_string()),
                None => (String::new(), "0".into()),
            },
            Mode::Endless => (self.en.score.to_string(), self.en_best.to_string()),
        };
        self.els.score.set_text_content(Some(&a));
        self.els.best.set_text_content(Some(&b));
    }

    fn render_status(&self) {
        let n = self.count_tiles();
        let left = format!("{} {}", n, if n == 1 { "TILE LEFT" } else { "TILES LEFT" });
        let (text, cls) = match (self.mode, &self.cz) {
            (Mode::Collatz | Mode::Daily, Some(cz)) if cz.stuck => {
                (format!("NO PATH{MIDDOT}UNDO TO CONTINUE"), "warn")
            }
            (Mode::Daily, Some(cz)) if cz.cleared => {
                let hist = Self::history_map();
                let best = hist
                    .get(cz.date.as_deref().unwrap_or(""))
                    .copied()
                    .unwrap_or(cz.moves);
                let streak = Self::streak_ending(&hist, cz.date.as_deref().unwrap_or(""));
                (
                    format!(
                        "CLEARED IN {}{MIDDOT}BEST {}{MIDDOT}STREAK {}",
                        cz.moves, best, streak
                    ),
                    "goal",
                )
            }
            (Mode::Collatz, Some(cz)) if cz.cleared => (
                format!("BOARD CLEARED{MIDDOT}BEST LEVEL {}", self.cz_best),
                "goal",
            ),
            (Mode::Daily, Some(cz)) => (
                format!("DAILY #{}{MIDDOT}{left}{MIDDOT}PAR {}", cz.level, cz.par),
                "goal",
            ),
            (Mode::Collatz, Some(cz)) => (
                format!("CLEAR THE BOARD{MIDDOT}{left}{MIDDOT}PAR {}", cz.par),
                "goal",
            ),
            (Mode::Endless, _) if self.en.over => (
                format!("BOARD JAMMED{MIDDOT}CLEARED {}", self.en.score),
                "warn",
            ),
            (Mode::Endless, _) => (
                format!(
                    "KEEP THE BOARD CLEAR{MIDDOT}{} {}",
                    n,
                    if n == 1 { "TILE" } else { "TILES" }
                ),
                "goal",
            ),
            _ => ("SWIPE OR USE ARROW KEYS".to_string(), ""),
        };
        self.els.status_line.set_text_content(Some(&text));
        let class = if cls.is_empty() {
            "status-line".to_string()
        } else {
            format!("status-line {cls}")
        };
        self.els.status_line.set_class_name(&class);
    }

    fn show_delta(&self, n: u32) {
        if n == 0 {
            return;
        }
        self.els.delta.set_text_content(Some(&format!("+{n}")));
        dom::set_class(&self.els.delta, "show", false);
        dom::reflow(&self.els.delta);
        dom::set_class(&self.els.delta, "show", true);
    }

    fn update_undo(&self) {
        let disabled = match self.mode {
            Mode::Classic => self.history.is_none() || self.over,
            Mode::Collatz | Mode::Daily => {
                self.cz.as_ref().map(|c| c.undo.is_empty()).unwrap_or(true)
            }
            Mode::Endless => self.en.history.is_none() || self.en.over,
        };
        self.els.undo.set_disabled(disabled);
    }

    fn announce(&self, msg: &str) {
        self.els.status.set_text_content(Some(msg));
    }

    fn shake(&self) {
        dom::set_class(&self.els.board, "shake", false);
        dom::reflow(&self.els.board);
        dom::set_class(&self.els.board, "shake", true);
    }

    // ---------- overlays ----------

    /// Anything that takes the intro off screen counts as having read it.
    fn dismiss_intro(&mut self) {
        if self.intro_open {
            self.intro_open = false;
            dom::local_set(CZ_INTRO_KEY, "1");
        }
    }

    fn set_overlay_btn(&mut self, slot: usize, spec: Option<OverlayButton>) {
        let btn = match slot {
            0 => &self.els.primary,
            1 => &self.els.secondary,
            _ => &self.els.alt,
        };
        match spec {
            Some(b) => {
                btn.set_hidden(false);
                btn.set_text_content(Some(b.label));
                self.actions[slot] = Some(b.action);
            }
            None => {
                btn.set_hidden(true);
                self.actions[slot] = None;
            }
        }
    }

    fn show_overlay(&mut self, spec: OverlaySpec) {
        self.dismiss_intro();
        self.els.overlay.set_hidden(false);
        dom::set_class(&self.els.overlay, "win", spec.win);
        dom::set_class(&self.els.overlay, "collatz", self.mode.is_cz());
        self.els.kicker.set_text_content(Some(&spec.kicker));
        self.els.title.set_text_content(Some(&spec.title));
        self.els.sub.set_text_content(Some(&spec.sub));
        dom::set_class(&self.els.sub, "wide", spec.wide);
        self.set_overlay_btn(0, spec.primary);
        self.set_overlay_btn(1, spec.secondary);
        self.set_overlay_btn(2, spec.alt);
        // DOM order follows visual order so tab order matches: Collatz overlays
        // lead with the primary action, 2048 overlays end with it.
        if let Some(actions) = self.els.primary.parent_node() {
            let order: [&HtmlButtonElement; 3] = if self.mode.is_cz() {
                [&self.els.primary, &self.els.secondary, &self.els.alt]
            } else {
                [&self.els.secondary, &self.els.alt, &self.els.primary]
            };
            for b in order {
                let _ = actions.append_child(b);
            }
        }
        if !spec.announce.is_empty() {
            self.announce(&spec.announce);
        }
    }

    fn hide_overlay(&mut self) {
        self.dismiss_intro();
        self.els.overlay.set_hidden(true);
    }

    fn overlay_visible(&self) -> bool {
        !self.els.overlay.hidden()
    }

    fn overlay_click(&mut self, slot: usize) {
        if let Some(action) = self.actions[slot] {
            self.run_action(action);
        }
    }

    fn run_action(&mut self, action: Action) {
        match action {
            Action::NewGame => self.new_game(),
            Action::KeepGoing => {
                self.keep_playing = true;
                self.hide_overlay();
                self.save_state();
            }
            Action::UndoCz => self.undo_cz(),
            Action::Retry => self.retry(),
            Action::NewBoard => self.new_board(),
            Action::NextLevel => {
                let next = self.cz.as_ref().map(|c| c.level + 1).unwrap_or(1);
                self.start_level(next, None, false);
            }
            Action::Share => self.share_daily(),
            Action::Begin => self.hide_overlay(),
            Action::NewRun => self.new_run_endless(),
        }
    }

    fn show_win_classic(&mut self) {
        self.show_overlay(OverlaySpec {
            win: true,
            kicker: "BASELINE TEST PASSED".into(),
            title: "MORE HUMAN THAN HUMAN".into(),
            sub: "YOU REACHED 2048. TYRELL WOULD BE PROUD.".into(),
            secondary: Some(OverlayButton {
                label: "KEEP GOING",
                action: Action::KeepGoing,
            }),
            primary: Some(OverlayButton {
                label: "NEW RUN",
                action: Action::NewGame,
            }),
            announce: "You reached 2048. More human than human.".into(),
            ..Default::default()
        });
    }

    fn show_lose_classic(&mut self) {
        let line = LOSS_LINES
            [(dom::random() * LOSS_LINES.len() as f64).floor() as usize % LOSS_LINES.len()];
        self.show_overlay(OverlaySpec {
            kicker: "RETIRED".into(),
            title: "TIME TO DIE".into(),
            sub: format!("{line}{MIDDOT}SCORE {}", self.score),
            primary: Some(OverlayButton {
                label: "NEW RUN",
                action: Action::NewGame,
            }),
            announce: format!("Game over. Final score {}.", self.score),
            ..Default::default()
        });
    }

    fn show_intro(&mut self) {
        self.show_overlay(OverlaySpec {
            win: true,
            kicker: "NEW MODE".into(),
            title: "COLLATZ".into(),
            sub: "PAIR EQUAL TILES. EVEN HALVES. ODD BECOMES 3N+1. PAIR 1s TO CLEAR. EMPTY THE BOARD.".into(),
            wide: true,
            primary: Some(OverlayButton { label: "BEGIN", action: Action::Begin }),
            announce: "Collatz mode. Pair equal tiles: even halves, odd becomes 3n plus 1, two 1s clear. Empty the board.".into(),
            ..Default::default()
        });
        self.intro_open = true;
    }

    fn show_stuck(&mut self) {
        dom::set_class(&self.els.board, "stuck", true);
        let daily = self.mode == Mode::Daily;
        self.show_overlay(OverlaySpec {
            kicker: "BASELINE DEVIATION".into(),
            title: "NO PATH".into(),
            sub: "THESE TILES CAN NO LONGER PAIR OFF".into(),
            primary: Some(OverlayButton {
                label: "UNDO",
                action: Action::UndoCz,
            }),
            secondary: Some(OverlayButton {
                label: "RETRY",
                action: Action::Retry,
            }),
            alt: if daily {
                None
            } else {
                Some(OverlayButton {
                    label: "NEW BOARD",
                    action: Action::NewBoard,
                })
            },
            announce: if daily {
                "No path to clear the board. Undo or retry.".into()
            } else {
                "No path to clear the board. Undo, retry, or take a new board.".into()
            },
            ..Default::default()
        });
    }

    fn tier(&self) -> &'static str {
        let cz = self.cz.as_ref().expect("cz state");
        if cz.moves <= cz.par {
            "PERFECT"
        } else if cz.moves <= cz.par + 2 {
            "CLEAN"
        } else {
            "CLEARED"
        }
    }

    fn show_cleared(&mut self) {
        let tier = self.tier().to_string();
        let cz = self.cz.clone().expect("cz state");
        let spec = if self.mode == Mode::Daily {
            let hist = Self::history_map();
            let date = cz.date.clone().unwrap_or_default();
            let mut sub = format!(
                "DAILY #{}{MIDDOT}{} MOVES{MIDDOT}PAR {}",
                cz.level, cz.moves, cz.par
            );
            if let Some(best) = hist.get(&date) {
                if *best < cz.moves {
                    sub.push_str(&format!("{MIDDOT}BEST {best}"));
                }
            }
            let streak = Self::streak_ending(&hist, &date);
            if streak > 1 {
                sub.push_str(&format!("{MIDDOT}STREAK {streak}"));
            }
            OverlaySpec {
                win: true,
                kicker: tier,
                title: "INTERLINKED".into(),
                sub,
                primary: Some(OverlayButton {
                    label: "SHARE",
                    action: Action::Share,
                }),
                secondary: Some(OverlayButton {
                    label: "RETRY",
                    action: Action::Retry,
                }),
                announce: format!(
                    "Daily board {} cleared in {} moves, par {}.",
                    cz.level, cz.moves, cz.par
                ),
                ..Default::default()
            }
        } else {
            let mut sub = format!(
                "LEVEL {}{MIDDOT}{} MOVES{MIDDOT}PAR {}",
                cz.level, cz.moves, cz.par
            );
            if cz.undos > 0 {
                sub.push_str(&format!(
                    "{MIDDOT}{} {}",
                    cz.undos,
                    if cz.undos == 1 { "UNDO" } else { "UNDOS" }
                ));
            }
            OverlaySpec {
                win: true,
                kicker: tier,
                title: "INTERLINKED".into(),
                sub,
                primary: Some(OverlayButton {
                    label: "NEXT LEVEL",
                    action: Action::NextLevel,
                }),
                announce: format!(
                    "Level {} cleared in {} moves, par {}.",
                    cz.level, cz.moves, cz.par
                ),
                ..Default::default()
            }
        };
        self.show_overlay(spec);
    }

    fn show_over_endless(&mut self) {
        self.show_overlay(OverlaySpec {
            kicker: "RETIRED".into(),
            title: "BOARD JAMMED".into(),
            sub: format!("CLEARED {}{MIDDOT}BEST {}", self.en.score, self.en_best),
            primary: Some(OverlayButton {
                label: "NEW RUN",
                action: Action::NewRun,
            }),
            announce: format!("Board jammed. {} tiles cleared.", self.en.score),
            ..Default::default()
        });
    }

    // ---------- move completion ----------

    /// Runs the pending end-of-move work now instead of when its timer fires, so
    /// a new game, an undo or a mode switch never races a slide in flight.
    fn flush_move(&mut self) {
        if let Some(id) = self.move_timer.take() {
            dom::clear_timeout(id);
        }
        self.queued_dir = None;
        if let Some(f) = self.pending_finish.take() {
            f(self);
        }
    }

    fn run_pending(&mut self) {
        self.move_timer = None;
        if let Some(f) = self.pending_finish.take() {
            f(self);
        }
    }

    fn schedule_finish(&mut self, f: Finish) {
        self.pending_finish = Some(f);
        if let Some(cb) = &self.timer_cb {
            self.move_timer = Some(dom::set_timeout(cb, MOVE_MS));
        }
    }

    /// Slides every tile to where the engine says it lands and returns the
    /// merges and clears the finish step must swap in.
    fn apply_tracked(&mut self, res: &MoveResult) -> Pending {
        let mut by_idx: Vec<Option<Tile>> = Vec::with_capacity(CELLS);
        for r in 0..SIZE {
            for c in 0..SIZE {
                by_idx.push(self.grid[r][c]);
            }
        }
        let mut new_grid: Grid = [[None; SIZE]; SIZE];
        let mut merges = Vec::new();
        let mut clears = Vec::new();
        for (i, src) in res.sources.iter().enumerate() {
            let (r, c) = (i / SIZE, i % SIZE);
            match src {
                None => {}
                Some(Source::One(a)) => {
                    let mut t = by_idx[*a].expect("source tile");
                    t.row = r;
                    t.col = c;
                    self.position_el(&t);
                    new_grid[r][c] = Some(t);
                }
                Some(Source::Two { ahead, mover }) => {
                    let mut a = by_idx[*ahead].expect("ahead tile");
                    let mut b = by_idx[*mover].expect("mover tile");
                    a.row = r;
                    a.col = c;
                    b.row = r;
                    b.col = c;
                    self.position_el(&a);
                    self.position_el(&b);
                    let merged = self.make_tile(res.cells[i], r, c);
                    new_grid[r][c] = Some(merged);
                    merges.push(Merge {
                        tile: merged,
                        sources: [a, b],
                    });
                }
            }
        }
        for cl in &res.clears {
            let (r, c) = (cl.at / SIZE, cl.at % SIZE);
            let mut a = by_idx[cl.a].expect("clear a");
            let mut b = by_idx[cl.b].expect("clear b");
            a.row = r;
            a.col = c;
            b.row = r;
            b.col = c;
            self.position_el(&a);
            self.position_el(&b);
            clears.push(ClearAnim { a, b });
        }
        self.grid = new_grid;
        Pending { merges, clears }
    }

    /// The end-of-move swap: merged tiles pop in, cleared pairs burst away.
    fn swap_in(&mut self, pending: &Pending) {
        for m in &pending.merges {
            for s in &m.sources {
                self.remove_el(s.id);
            }
            let el = self.tile_el(m.tile);
            dom::set_class(&el, "merged", true);
        }
        for cl in &pending.clears {
            self.remove_el(cl.b.id);
            if let Some(el) = self.elements.remove(&cl.a.id) {
                dom::set_class(&el, "cleared", true);
                let cb = Closure::once_into_js(move || {
                    el.remove();
                });
                let _ = dom::window()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 420);
            }
        }
    }

    fn drain_queue(&mut self, allowed: bool) {
        let next = self.queued_dir.take();
        if let (Some(dir), true) = (next, allowed) {
            self.handle_move(dir);
        }
    }

    // ---------- 2048 flow ----------

    fn new_game(&mut self) {
        self.flush_move();
        self.grid = [[None; SIZE]; SIZE];
        self.score = 0;
        self.won = false;
        self.keep_playing = false;
        self.over = false;
        self.history = None;
        self.animating = false;
        self.queued_dir = None;
        self.hide_overlay();
        self.spawn_classic();
        self.spawn_classic();
        self.render_all();
        self.render_status();
        self.update_undo();
        self.save_state();
        self.announce("New game started.");
    }

    fn spawn_classic(&mut self) -> Option<Tile> {
        let mut cells = self.cells();
        let (idx, value) = classic::spawn(&mut cells, &mut dom::random)?;
        let t = self.make_tile(value, idx / SIZE, idx % SIZE);
        self.grid[idx / SIZE][idx % SIZE] = Some(t);
        Some(t)
    }

    fn undo_classic(&mut self) {
        self.flush_move();
        let Some(h) = self.history.take() else { return };
        self.restore(&h);
        self.hide_overlay();
        self.render_all();
        self.update_undo();
        self.save_state();
        self.announce("Move undone.");
    }

    fn handle_move_classic(&mut self, dir: u8) {
        let snap = self.snapshot();
        let res = classic::apply_move(&self.cells(), dir);
        if !res.moved {
            self.shake();
            return;
        }
        self.history = Some(snap);
        self.animating = true;
        let pending = self.apply_tracked(&res);

        self.score += res.gained;
        if self.score > self.best {
            self.best = self.score;
            dom::local_set(CLASSIC_BEST_KEY, &self.best.to_string());
        }
        let reached_win = !self.won && pending.merges.iter().any(|m| m.tile.value >= WIN_VALUE);
        let gained = res.gained;

        self.schedule_finish(Box::new(move |app: &mut App| {
            app.swap_in(&pending);
            if let Some(t) = app.spawn_classic() {
                let el = app.tile_el(t);
                dom::set_class(&el, "new", true);
            }
            app.render_score();
            app.show_delta(gained);
            if reached_win {
                app.won = true;
                if !app.keep_playing {
                    app.show_win_classic();
                }
            }
            if !classic::moves_available(&app.cells()) {
                app.over = true;
                app.show_lose_classic();
            }
            app.animating = false;
            app.update_undo();
            app.save_state();
            app.drain_queue(true);
        }));
    }

    // ---------- Collatz flow ----------

    /// Puts a fresh board on screen and resets the per-board state.
    fn begin_board(&mut self, level: u32, date: Option<String>, cells: Cells, par: u32) {
        self.flush_move();
        self.cz = Some(CzState {
            level,
            date,
            moves: 0,
            par,
            start: cells,
            undo: Vec::new(),
            undos: 0,
            stuck: false,
            cleared: false,
        });
        self.grid = self.grid_from(&cells);
        self.animating = false;
        self.queued_dir = None;
        self.hide_overlay();
        dom::set_class(&self.els.board, "stuck", false);
        self.render_all();
        self.render_status();
        self.update_undo();
        self.save_state();
        let what = if self.mode == Mode::Daily {
            format!("Daily board {level}")
        } else {
            format!("Level {level}")
        };
        self.announce(&format!("{what}. {} tiles. Par {par}.", self.count_tiles()));
    }

    /// redeal: an explicit NEW BOARD, which on level 1 skips the fixed teaching board.
    fn start_level(&mut self, level: u32, start: Option<Cells>, redeal: bool) {
        let (cells, par) = match start {
            Some(cells) => {
                let sol = collatz::solve(&cells, 0);
                (
                    cells,
                    if sol.solved {
                        sol.moves.len() as u32
                    } else {
                        0
                    },
                )
            }
            None => {
                let mut p = collatz::params(level);
                if redeal {
                    p.fixed = false;
                }
                let g = collatz::generate_with(p, &mut dom::random);
                (g.cells, g.par)
            }
        };
        self.begin_board(level, None, cells, par);
    }

    fn start_daily(&mut self) {
        let key = dom::today_key();
        let d = daily::daily(&key);
        self.begin_board(d.day.max(0) as u32, Some(key), d.board.cells, d.board.par);
    }

    /// The same board again, from the top.
    fn restart_board(&mut self) {
        if let Some(cz) = self.cz.clone() {
            self.begin_board(cz.level, cz.date, cz.start, cz.par);
        }
    }

    fn retry(&mut self) {
        match self.mode {
            Mode::Collatz | Mode::Daily => self.restart_board(),
            Mode::Classic => self.new_game(),
            Mode::Endless => self.new_run_endless(),
        }
    }

    /// After a clear, NEW BOARD means the next level, same as the overlay button.
    fn new_board(&mut self) {
        match self.mode {
            Mode::Daily => self.restart_board(),
            Mode::Collatz => {
                let (level, cleared) = self
                    .cz
                    .as_ref()
                    .map(|c| (c.level, c.cleared))
                    .unwrap_or((1, false));
                if cleared {
                    self.start_level(level + 1, None, false);
                } else {
                    self.start_level(level, None, true);
                }
            }
            _ => {}
        }
    }

    fn undo_cz(&mut self) {
        self.flush_move();
        let Some(cz) = self.cz.as_mut() else { return };
        let Some((cells, moves)) = cz.undo.pop() else {
            return;
        };
        cz.moves = moves;
        cz.undos += 1;
        cz.stuck = false;
        cz.cleared = false;
        self.grid = self.grid_from(&cells);
        self.hide_overlay();
        dom::set_class(&self.els.board, "stuck", false);
        self.render_all();
        self.render_status();
        self.update_undo();
        self.save_state();
        self.announce("Move undone.");
    }

    fn share_text(&self) -> String {
        let cz = self.cz.as_ref().expect("cz state");
        format!(
            "2049 DAILY #{}{MIDDOT}{} MOVES{MIDDOT}PAR {}{MIDDOT}{}\n{}",
            cz.level,
            cz.moves,
            cz.par,
            self.tier(),
            dom::page_url()
        )
    }

    fn share_daily(&mut self) {
        let text = self.share_text();
        let nav = dom::window().navigator();
        let weak = self.me.clone();
        // Feature detection through Reflect keeps this independent of web-sys bindings.
        if let Ok(share) = js_sys::Reflect::get(&nav, &"share".into()) {
            if let Some(share) = share.dyn_ref::<js_sys::Function>() {
                let data = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&data, &"text".into(), &text.clone().into());
                if let Ok(p) = share.call1(&nav, &data) {
                    if let Ok(p) = p.dyn_into::<js_sys::Promise>() {
                        wasm_bindgen_futures::spawn_local(async move {
                            if JsFuture::from(p).await.is_ok() {
                                with_app(&weak, |a| a.els.primary.set_text_content(Some("SHARED")));
                            }
                        });
                        return;
                    }
                }
            }
        }
        if let Ok(clip) = js_sys::Reflect::get(&nav, &"clipboard".into()) {
            if !clip.is_undefined() && !clip.is_null() {
                if let Ok(write) = js_sys::Reflect::get(&clip, &"writeText".into()) {
                    if let Some(write) = write.dyn_ref::<js_sys::Function>() {
                        if let Ok(p) = write.call1(&clip, &text.clone().into()) {
                            if let Ok(p) = p.dyn_into::<js_sys::Promise>() {
                                wasm_bindgen_futures::spawn_local(async move {
                                    let label = if JsFuture::from(p).await.is_ok() {
                                        "COPIED"
                                    } else {
                                        "COPY FAILED"
                                    };
                                    with_app(&weak, |a| {
                                        a.els.primary.set_text_content(Some(label))
                                    });
                                });
                                return;
                            }
                        }
                    }
                }
            }
        }
        let _ = dom::window().prompt_with_message_and_default("Copy your result", &text);
    }

    fn level_cleared(&mut self) {
        let Some(cz) = self.cz.as_mut() else { return };
        cz.cleared = true;
        let (level, moves, date) = (cz.level, cz.moves, cz.date.clone());
        if self.mode == Mode::Daily {
            let mut hist = Self::history_map();
            let key = date.unwrap_or_default();
            let entry = hist.entry(key).or_insert(moves);
            if moves < *entry {
                *entry = moves;
            }
            dom::local_set(DAILY_HIST_KEY, &to_json(&hist));
        } else if level > self.cz_best {
            self.cz_best = level;
            dom::local_set(CZ_BEST_KEY, &self.cz_best.to_string());
        }
        self.show_cleared();
    }

    fn handle_move_cz(&mut self, dir: u8) {
        let before = self.cells();
        let res = collatz::apply_move(&before, dir, true);
        if !res.moved {
            self.shake();
            return;
        }
        {
            let cz = self.cz.as_mut().expect("cz state");
            cz.undo.push((before, cz.moves));
            if cz.undo.len() > UNDO_CAP {
                cz.undo.remove(0);
            }
            cz.moves += 1;
        }
        self.animating = true;
        let pending = self.apply_tracked(&res);

        self.schedule_finish(Box::new(move |app: &mut App| {
            app.swap_in(&pending);
            app.render_score();
            let cells = app.cells();
            let mut blocked = false;
            if collatz::is_empty(&cells) {
                app.level_cleared();
                blocked = true;
            } else {
                let sol = collatz::solve(&cells, 0);
                if !sol.solved && !sol.exhausted {
                    if let Some(cz) = app.cz.as_mut() {
                        cz.stuck = true;
                    }
                    app.show_stuck();
                    blocked = true;
                }
            }
            app.render_status();
            app.animating = false;
            app.update_undo();
            app.save_state();
            app.drain_queue(!blocked);
        }));
    }

    // ---------- Endless flow ----------

    /// Mostly small evens, some 1s to clear with, a few odds to make trouble.
    fn spawn_endless(&mut self) -> Option<Tile> {
        let mut empties = Vec::new();
        for r in 0..SIZE {
            for c in 0..SIZE {
                if self.grid[r][c].is_none() {
                    empties.push((r, c));
                }
            }
        }
        if empties.is_empty() {
            return None;
        }
        let (r, c) =
            empties[(dom::random() * empties.len() as f64).floor() as usize % empties.len()];
        let x = dom::random();
        let value = if x < 0.15 {
            1
        } else if x < 0.55 {
            2
        } else if x < 0.65 {
            3
        } else if x < 0.90 {
            4
        } else {
            8
        };
        let t = self.make_tile(value, r, c);
        self.grid[r][c] = Some(t);
        Some(t)
    }

    fn new_run_endless(&mut self) {
        self.flush_move();
        self.grid = [[None; SIZE]; SIZE];
        self.en = EndlessState::default();
        self.animating = false;
        self.queued_dir = None;
        self.hide_overlay();
        for _ in 0..3 {
            self.spawn_endless();
        }
        self.render_all();
        self.render_status();
        self.update_undo();
        self.save_state();
        self.announce("New endless run started.");
    }

    fn undo_endless(&mut self) {
        self.flush_move();
        let Some((cells, score)) = self.en.history.take() else {
            return;
        };
        self.grid = self.grid_from(&cells);
        self.en.score = score;
        self.en.over = false;
        self.hide_overlay();
        self.render_all();
        self.render_status();
        self.update_undo();
        self.save_state();
        self.announce("Move undone.");
    }

    fn handle_move_endless(&mut self, dir: u8) {
        let before = self.cells();
        let res = collatz::apply_move(&before, dir, true);
        if !res.moved {
            self.shake();
            return;
        }
        self.en.history = Some((before, self.en.score));
        self.animating = true;
        let pending = self.apply_tracked(&res);
        // Tiles removed this move: one per merge, one more per cleared pair.
        let removed = res.merges + res.cleared;
        self.en.score += removed;
        if self.en.score > self.en_best {
            self.en_best = self.en.score;
            dom::local_set(ENDLESS_BEST_KEY, &self.en_best.to_string());
        }

        self.schedule_finish(Box::new(move |app: &mut App| {
            app.swap_in(&pending);
            if let Some(t) = app.spawn_endless() {
                let el = app.tile_el(t);
                dom::set_class(&el, "new", true);
            }
            app.render_score();
            app.show_delta(removed);
            let mut blocked = false;
            if !classic::moves_available(&app.cells()) {
                app.en.over = true;
                app.show_over_endless();
                blocked = true;
            }
            app.render_status();
            app.animating = false;
            app.update_undo();
            app.save_state();
            app.drain_queue(!blocked);
        }));
    }

    // ---------- dispatch ----------

    fn handle_move(&mut self, dir: u8) {
        if self.overlay_visible() {
            return;
        }
        if (self.mode == Mode::Classic && self.over) || (self.mode == Mode::Endless && self.en.over)
        {
            return;
        }
        if self.animating {
            self.queued_dir = Some(dir);
            return;
        }
        match self.mode {
            Mode::Classic => self.handle_move_classic(dir),
            Mode::Collatz | Mode::Daily => self.handle_move_cz(dir),
            Mode::Endless => self.handle_move_endless(dir),
        }
    }

    fn undo(&mut self) {
        match self.mode {
            Mode::Classic => self.undo_classic(),
            Mode::Collatz | Mode::Daily => self.undo_cz(),
            Mode::Endless => self.undo_endless(),
        }
    }

    fn primary_action(&mut self) {
        match self.mode {
            Mode::Classic => self.new_game(),
            Mode::Collatz | Mode::Daily => self.new_board(),
            Mode::Endless => self.new_run_endless(),
        }
    }

    // ---------- theme ----------

    /// Applies a theme: the data-theme attribute the stylesheet keys on, the
    /// switch state, the browser chrome colour, and the saved preference.
    fn set_theme(&mut self, t: Theme, boot: bool) {
        if !boot && t == self.theme {
            return;
        }
        self.theme = t;
        // Boot only reflects a stored choice; a first visit stays unwritten.
        if !boot {
            dom::local_set(THEME_KEY, t.key());
        }
        let doc = dom::document();
        if let Some(root) = doc.document_element() {
            let _ = root.set_attribute("data-theme", t.key());
        }
        if let Ok(Some(meta)) = doc.query_selector("meta[name=theme-color]") {
            let _ = meta.set_attribute("content", t.chrome());
        }
        for (i, b) in self.els.theme_btns.iter().enumerate() {
            let _ = b.set_attribute(
                "aria-checked",
                if THEMES[i] == t { "true" } else { "false" },
            );
        }
        if !boot {
            self.announce(if t == Theme::Classic {
                "Classic theme."
            } else {
                "Neon theme."
            });
        }
    }

    // ---------- mode switch ----------

    fn set_mode(&mut self, m: Mode, boot: bool) {
        if !boot {
            if m == self.mode {
                return;
            }
            self.flush_move();
            self.save_state();
        }
        self.mode = m;
        dom::local_set(MODE_KEY, m.key());
        self.animating = false;
        self.queued_dir = None;

        let cz = m.is_cz();
        for (i, b) in self.els.mode_btns.iter().enumerate() {
            let _ = b.set_attribute("aria-checked", if MODES[i] == m { "true" } else { "false" });
        }
        dom::set_class(&self.els.board, "collatz", cz);
        dom::set_class(&self.els.board, "stuck", false);
        let (tagline, score_label, best_label, primary) = match m {
            Mode::Classic => ("MORE HUMAN THAN HUMAN", "SCORE", "BEST", "NEW RUN"),
            Mode::Collatz => ("CELLS INTERLINKED", "LEVEL", "MOVES", "NEW BOARD"),
            Mode::Daily => ("DAILY BASELINE TEST", "DAY", "MOVES", "RETRY"),
            Mode::Endless => ("TEARS IN RAIN", "CLEARED", "BEST", "NEW RUN"),
        };
        self.els.tagline.set_text_content(Some(tagline));
        self.els.score_label.set_text_content(Some(score_label));
        self.els.best_label.set_text_content(Some(best_label));
        self.els.new_game.set_text_content(Some(primary));
        self.hide_overlay();

        match m {
            Mode::Classic => {
                if self.load_classic() {
                    self.render_all();
                    if self.won && !self.keep_playing {
                        self.show_win_classic();
                    }
                } else {
                    self.new_game();
                }
            }
            Mode::Collatz | Mode::Daily => {
                if self.load_cz() {
                    self.render_all();
                    let (cleared, stuck) = self
                        .cz
                        .as_ref()
                        .map(|c| (c.cleared, c.stuck))
                        .unwrap_or((false, false));
                    if cleared {
                        self.show_cleared();
                    } else if stuck {
                        self.show_stuck();
                    }
                } else if m == Mode::Daily {
                    self.start_daily();
                } else {
                    self.start_level(1, None, false);
                }
            }
            Mode::Endless => {
                if self.load_endless() {
                    self.render_all();
                } else {
                    self.new_run_endless();
                }
            }
        }
        // Never cover a restored cleared or stuck overlay with the intro.
        if cz && !self.overlay_visible() && dom::local_get(CZ_INTRO_KEY).is_none() {
            self.show_intro();
        }
        self.render_status();
        self.update_undo();
    }

    // ---------- input ----------

    fn on_key(&mut self, e: &KeyboardEvent) {
        if e.alt_key() || e.ctrl_key() || e.meta_key() {
            return;
        }
        let key = e.key();
        let dir = match key.as_str() {
            "ArrowUp" | "w" | "W" | "k" => Some(0),
            "ArrowRight" | "d" | "D" | "l" => Some(1),
            "ArrowDown" | "s" | "S" | "j" => Some(2),
            "ArrowLeft" | "a" | "A" | "h" => Some(3),
            _ => None,
        };
        match (dir, key.as_str()) {
            (Some(d), _) => {
                e.prevent_default();
                self.handle_move(d);
            }
            (None, "z") if !e.shift_key() => {
                e.prevent_default();
                self.undo();
            }
            (None, "r" | "R") => {
                e.prevent_default();
                self.retry();
            }
            // N is a Collatz shortcut only; 2048 keeps its Z and R contract.
            (None, "n" | "N") if self.mode.is_puzzle() => {
                e.prevent_default();
                self.new_board();
            }
            // Key repeat would strobe the page while T is held.
            (None, "t" | "T") if !e.repeat() => {
                e.prevent_default();
                self.set_theme(self.theme.other(), false);
            }
            _ => {}
        }
    }

    fn on_start(&mut self, x: f64, y: f64) {
        self.touch_x = x;
        self.touch_y = y;
        self.touching = true;
    }

    /// A direction once the gesture passes the threshold, otherwise None.
    fn swipe_dir(&self, x: f64, y: f64) -> Option<u8> {
        let (dx, dy) = (x - self.touch_x, y - self.touch_y);
        let (ax, ay) = (dx.abs(), dy.abs());
        if ax.max(ay) < SWIPE_MIN {
            return None;
        }
        Some(if ax > ay {
            if dx > 0.0 {
                1
            } else {
                3
            }
        } else if dy > 0.0 {
            2
        } else {
            0
        })
    }

    fn on_move(&mut self, x: f64, y: f64) {
        if !self.touching {
            return;
        }
        if let Some(dir) = self.swipe_dir(x, y) {
            self.touching = false;
            self.handle_move(dir);
        }
    }

    fn on_end(&mut self, x: f64, y: f64) {
        self.on_move(x, y);
        self.touching = false;
    }
}
