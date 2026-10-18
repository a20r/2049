//! Thin helpers over web-sys so the game code reads like the DOM it drives.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlButtonElement, HtmlElement, Storage, Window};

pub fn window() -> Window {
    web_sys::window().expect("no window")
}

pub fn document() -> Document {
    window().document().expect("no document")
}

/// An element by id, as an HtmlElement. Panics on a missing id: the page is ours.
pub fn el(id: &str) -> HtmlElement {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("missing element #{id}"))
        .dyn_into::<HtmlElement>()
        .unwrap_or_else(|_| panic!("#{id} is not an HtmlElement"))
}

pub fn btn(id: &str) -> HtmlButtonElement {
    el(id)
        .dyn_into::<HtmlButtonElement>()
        .unwrap_or_else(|_| panic!("#{id} is not a button"))
}

pub fn create(tag: &str, class: &str) -> HtmlElement {
    let e = document()
        .create_element(tag)
        .expect("create_element")
        .dyn_into::<HtmlElement>()
        .expect("HtmlElement");
    e.set_class_name(class);
    e
}

pub fn set_class(e: &Element, name: &str, on: bool) {
    let _ = e.class_list().toggle_with_force(name, on);
}

pub fn set_var(e: &HtmlElement, name: &str, value: &str) {
    let _ = e.style().set_property(name, value);
}

/// Forces a reflow so a re-added animation class restarts.
pub fn reflow(e: &HtmlElement) {
    let _ = e.offset_width();
}

pub fn set_timeout(cb: &Closure<dyn FnMut()>, ms: i32) -> i32 {
    window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), ms)
        .expect("setTimeout")
}

pub fn clear_timeout(id: i32) {
    window().clear_timeout_with_handle(id);
}

fn storage() -> Option<Storage> {
    window().local_storage().ok().flatten()
}

/// localStorage reads and writes never fail the game: a blocked store just means no persistence.
pub fn local_get(key: &str) -> Option<String> {
    storage().and_then(|s| s.get_item(key).ok().flatten())
}

pub fn local_set(key: &str, value: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, value);
    }
}

pub fn local_remove(key: &str) {
    if let Some(s) = storage() {
        let _ = s.remove_item(key);
    }
}

/// Today's local calendar date as YYYY-MM-DD.
pub fn today_key() -> String {
    let d = js_sys::Date::new_0();
    nexus_engine::daily::date_key(d.get_full_year() as i32, d.get_month() + 1, d.get_date())
}

pub fn random() -> f64 {
    js_sys::Math::random()
}

/// The page URL without query or hash, for the share text.
pub fn page_url() -> String {
    let loc = window().location();
    format!(
        "{}{}",
        loc.origin().unwrap_or_default(),
        loc.pathname().unwrap_or_default()
    )
}
