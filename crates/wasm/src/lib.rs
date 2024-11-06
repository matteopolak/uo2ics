#![warn(clippy::pedantic)]

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(js_name = fromHtml)]
#[must_use]
pub fn from_html(html: &[u8]) -> String {
	let courses = uo2ics_core::course::parse_from_buf(html);
	let calendar = uo2ics_core::create_calendar(courses);

	calendar.to_string()
}
