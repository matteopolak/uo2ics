#![feature(iter_array_chunks)]

use std::{fmt, fs::File, io::Write, str::FromStr};

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use icalendar::{Calendar, Component, Event, EventLike};
use select::{
	document::Document,
	node::Node,
	predicate::{self, Element, Name},
};

const TimeZone: Tz = chrono_tz::Canada::Eastern;

#[derive(Debug)]
pub enum CourseStatus {
	Enrolled,
	Waiting,
}

impl FromStr for CourseStatus {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"Enrolled" => Self::Enrolled,
			"Waiting" => Self::Waiting,
			_ => return Err(()),
		})
	}
}

#[derive(Debug)]
pub struct Course {
	pub name: String,
	pub status: CourseStatus,
	pub classes: Vec<Class>,
}

#[derive(Debug, Clone, Copy)]
pub struct Section(char, u8);

impl fmt::Display for Section {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}{:02}", self.0, self.1)
	}
}

impl FromStr for Section {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut chars = s.chars();
		let letter = chars.next().ok_or(())?;
		let number = chars.collect::<String>().parse().map_err(|_| ())?;

		assert!(number < 100, "section number >= 100");

		Ok(Self(letter, number))
	}
}

#[derive(Debug, Clone, Copy)]
pub enum CourseComponent {
	Laboratory,
	Lecture,
	Tutorial,
}

impl fmt::Display for CourseComponent {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", match self {
			Self::Laboratory => "LAB",
			Self::Lecture => "LEC",
			Self::Tutorial => "TUT",
		})
	}
}

impl FromStr for CourseComponent {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"Laboratory" => Self::Laboratory,
			"Lecture" => Self::Lecture,
			"Tutorial" => Self::Tutorial,
			_ => return Err(()),
		})
	}
}

#[derive(Debug, Clone)]
pub struct Class {
	pub section: Section,
	pub component: CourseComponent,
	pub time: DateTimeRange,
	pub location: String,
	pub address: String,
	pub instructor: String,
}

const WEEKDAYS: [&str; 5] = ["Mo", "Tu", "We", "Th", "Fr"];

#[derive(Debug)]
pub struct DateTimeRangeRaw {
	pub start: (u8, u8),
	pub end: (u8, u8),
	pub weekday: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct DateTimeRange {
	pub start: DateTime<Tz>,
	pub end: DateTime<Tz>,
}

fn parse_time(s: &str) -> Result<(u8, u8), ()> {
	let mut chars = s.chars();
	let mut hour = chars
		.by_ref()
		.take_while(|&c| c != ':')
		.collect::<String>()
		.parse()
		.map_err(|_| ())?;
	let minute = chars
		.by_ref()
		.take_while(|&c| c != 'A' && c != 'P')
		.collect::<String>()
		.parse()
		.map_err(|_| ())?;
	let am_pm = chars.collect::<String>();

	if am_pm == "PM" {
		hour += 12;
	}

	Ok((hour, minute))
}

impl FromStr for DateTimeRangeRaw {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut parts = s.splitn(2, " ");
		// day of week. 0 = monday
		let day = parts
			.next()
			.ok_or(())
			.and_then(|d| WEEKDAYS.iter().position(|&weekday| weekday == d).ok_or(()))?;
		// X:XXAM/PM - X:XXAM/PM
		let mut time = parts.next().ok_or(())?.split(" - ");
		let start = time.next().ok_or(())?;
		let end = time.next().ok_or(())?;

		let start = parse_time(start)?;
		let end = parse_time(end)?;

		Ok(Self {
			start,
			end,
			weekday: day as u8,
		})
	}
}

impl DateTimeRangeRaw {
	/// Convert the raw time range into a `DateTimeRange`, where the first day is
	/// the first day of the upcoming school year.
	pub fn into_datetime_range(self, first_day: DateTime<Tz>) -> DateTimeRange {
		let mut start = first_day;
		let mut end = first_day;

		start = start.with_hour(self.start.0 as u32).unwrap();
		start = start.with_minute(self.start.1 as u32).unwrap();
		start = start.with_second(0).unwrap();
		start = start.with_nanosecond(0).unwrap();

		end = end.with_hour(self.end.0 as u32).unwrap();
		end = end.with_minute(self.end.1 as u32).unwrap();
		end = end.with_second(0).unwrap();
		end = end.with_nanosecond(0).unwrap();

		let curr_weekday = start.weekday();

		// figure out how many days to add to get to the correct weekday
		let days_to_add = if curr_weekday.num_days_from_monday() > self.weekday as u32 {
			7 - curr_weekday.num_days_from_monday() + self.weekday as u32
		} else {
			self.weekday as u32 - curr_weekday.num_days_from_monday()
		};

		start += chrono::Duration::days(days_to_add as i64);
		end += chrono::Duration::days(days_to_add as i64);

		DateTimeRange { start, end }
	}
}

fn parse_from_file() -> (Vec<Course>, DateTime<Tz>) {
	let file = File::open("SA_LEARNER_SERVICES.html").unwrap();
	let document = Document::from_read(file).unwrap();

	// first wednesday of the upcoming school year. school starts in September (fall
	// term), January (winter term), and May (summer term) -- pick the closest one
	let month = Utc::now().with_timezone(&TimeZone).month();
	let (month, rollover_year) = if month <= 5 {
		(5, false)
	} else if month <= 9 {
		(9, false)
	} else {
		(1, true)
	};

	let mut date = TimeZone
		.with_ymd_and_hms(
			chrono::Utc::now().with_timezone(&TimeZone).year() + rollover_year as i32,
			month,
			1,
			0,
			0,
			0,
		)
		.unwrap();

	if date.weekday() != chrono::Weekday::Wed {
		let days_to_add = (2 + 7 - date.weekday().num_days_from_monday()) as i64 % 7;
		date += chrono::Duration::days(days_to_add);
	}

	let year_end = date + chrono::Duration::weeks(16);

	let mut courses = Vec::new();

	for node in document.find(predicate::Class("PAGROUPDIVIDER")) {
		let class = node.parent().unwrap().parent().unwrap();
		let mut rows = class.find(predicate::Class("PSLEVEL3GRID"));

		let mut head = rows.next().unwrap().find(Name("td")).map(|s| {
			s.find(Name("span"))
				.next()
				.map_or_else(|| String::from("\u{a0}"), |s| s.text())
		});

		let status: CourseStatus = head.next().unwrap().parse().unwrap();

		let name = node.text().split(" - ").next().unwrap().to_string();
		let cols = rows
			.next()
			.unwrap()
			.find(Name("td"))
			.map(|s| {
				s.find(Name("span"))
					.next()
					.map_or_else(|| String::from("\u{a0}"), |s| s.text())
			})
			.array_chunks::<7>();

		let mut prev = None::<Class>;

		let classes = cols
			.map(|[_, section, component, time, location, instructor, _]| {
				let section: Section = if section == "\u{a0}" {
					prev.as_ref().unwrap().section
				} else {
					section.parse().unwrap()
				};
				let component = if component == "\u{a0}" {
					prev.as_ref().unwrap().component
				} else {
					component.parse().unwrap()
				};
				let time: DateTimeRangeRaw = time.parse().unwrap();
				let mut room = location.splitn(2, " (");
				let address = room.next().unwrap().to_string();
				let location = room.next().unwrap().to_string().replace(")", "");

				let class = Class {
					section,
					component,
					time: time.into_datetime_range(date),
					location,
					address,
					instructor,
				};

				prev = Some(class.clone());
				class
			})
			.collect::<Vec<_>>();

		courses.push(Course {
			name,
			status,
			classes,
		});
	}

	(courses, year_end)
}

fn main() {
	let (courses, year_end) = parse_from_file();
	println!("{:#?}", courses);

	let mut calendar = Calendar::new();

	calendar.name("University of Ottawa");

	for course in courses {
		for class in course.classes {
			let mut event = Event::new();

			event.summary(&format!("{} {}", class.location, course.name));

			let start = class.time.start;
			let end = class.time.end;

			event
				.starts(start.to_utc() + chrono::Duration::hours(4))
				.ends(end.to_utc() + chrono::Duration::hours(4))
				.location(&format!("{}, Ottawa, ON, Canada", class.location))
				.description(&format!(
					"Section: {}\nInstructor: {}",
					class.section, class.instructor
				))
				// make it repeat weekly
				.add_property("RRULE", format!("FREQ=WEEKLY;UNTIL={}", year_end));

			calendar.push(event);
		}
	}

	let mut file = File::create("uottawa.ics").unwrap();
	write!(&mut file, "{}", calendar).unwrap();
}
