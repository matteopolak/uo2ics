#![warn(clippy::pedantic)]

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
#[must_use]
pub fn from_html(html: &str) -> String {
	let courses = uo2ics_core::course::parse_from_buf(html.as_bytes());
	let calendar = uo2ics_core::create_calendar(courses);

	calendar.to_string()
}
