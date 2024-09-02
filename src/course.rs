use std::{fmt, fs::File, path::Path, str::FromStr};

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeZone, Timelike};
use chrono_tz::Tz;
use select::{
	document::Document,
	predicate::{self, Name},
};

use crate::TZ;

#[derive(Debug)]
pub enum Status {
	Enrolled,
	Waiting,
}

impl FromStr for Status {
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
	pub code: String,
	pub status: Status,
	pub classes: Vec<Class>,
}

#[derive(Clone, Copy)]
pub struct Section(char, u8);

impl fmt::Debug for Section {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		fmt::Display::fmt(self, f)
	}
}

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
pub enum Component {
	Laboratory,
	Lecture,
	Tutorial,
}

impl fmt::Display for Component {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", match self {
			Self::Laboratory => "LAB",
			Self::Lecture => "LEC",
			Self::Tutorial => "TUT",
		})
	}
}

impl FromStr for Component {
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
	pub component: Component,
	pub time: DateTimeRange,
	pub location: String,
	pub address: String,
	pub instructor: String,

	pub end: DateTime<Tz>,
}

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
	let mut ap = ' ';
	let mut chars = s.chars();
	let mut hour = chars
		.by_ref()
		.take_while(|&c| c != ':')
		.collect::<String>()
		.parse()
		.map_err(|_| ())?;
	let minute = chars
		.by_ref()
		.take_while(|&c| {
			if c == 'A' || c == 'P' {
				ap = c;
				false
			} else {
				true
			}
		})
		.collect::<String>()
		.parse()
		.map_err(|_| ())?;

	if ap == 'P' && hour != 12 {
		hour += 12;
	}

	if ap == 'A' && hour == 12 {
		hour = 0;
	}

	Ok((hour, minute))
}

const WEEKDAYS: [&str; 5] = ["Mo", "Tu", "We", "Th", "Fr"];

impl FromStr for DateTimeRangeRaw {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut parts = s.splitn(2, ' ');
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
			weekday: u8::try_from(day).unwrap(),
		})
	}
}

impl DateTimeRangeRaw {
	/// Convert the raw time range into a `DateTimeRange`, where the first day is
	/// the first day of the upcoming school year.
	pub fn into_datetime_range(self, first_day: DateTime<Tz>) -> DateTimeRange {
		let mut start = first_day;
		let mut end = first_day;

		start = start.with_hour(u32::from(self.start.0)).unwrap();
		start = start.with_minute(u32::from(self.start.1)).unwrap();
		start = start.with_second(0).unwrap();
		start = start.with_nanosecond(0).unwrap();

		end = end.with_hour(u32::from(self.end.0)).unwrap();
		end = end.with_minute(u32::from(self.end.1)).unwrap();
		end = end.with_second(0).unwrap();
		end = end.with_nanosecond(0).unwrap();

		let curr_weekday = start.weekday();

		// figure out how many days to add to get to the correct weekday
		let days_to_add = if curr_weekday.num_days_from_monday() > u32::from(self.weekday) {
			7 - curr_weekday.num_days_from_monday() + u32::from(self.weekday)
		} else {
			u32::from(self.weekday) - curr_weekday.num_days_from_monday()
		};

		start += chrono::Duration::days(i64::from(days_to_add));
		end += chrono::Duration::days(i64::from(days_to_add));

		DateTimeRange { start, end }
	}
}

pub fn parse_from_file<P: AsRef<Path>>(path: Option<P>) -> Vec<Course> {
	let document = if let Some(path) = path {
		let file = File::open(path).unwrap();
		Document::from_read(file).unwrap()
	} else {
		Document::from_read(std::io::stdin()).unwrap()
	};

	let mut courses = Vec::new();

	for node in document.find(predicate::Class("PAGROUPDIVIDER")) {
		let class = node.parent().unwrap().parent().unwrap();
		let mut rows = class.find(predicate::Class("PSLEVEL3GRID"));

		let mut head = rows.next().unwrap().find(Name("td")).map(|s| {
			s.find(Name("span"))
				.next()
				.map_or_else(|| String::from("\u{a0}"), |s| s.text())
		});

		let status = head.next().unwrap().parse().unwrap();

		let title = node.text();
		let mut title = title.split(" - ");
		let code = title.next().unwrap().to_string();
		let name = title.next().unwrap().to_string();
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
			.map(
				|[_, section, component, time, location, instructor, start_end]| {
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
					let location = room.next().unwrap().to_string().replace(')', "");

					let mut start_end = start_end.split(" - ");
					let start = TZ.from_utc_datetime(
						&NaiveDate::parse_from_str(start_end.next().unwrap(), "%m/%d/%Y")
							.unwrap()
							.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
					) + chrono::Duration::hours(4);
					let end = TZ.from_utc_datetime(
						&NaiveDate::parse_from_str(start_end.next().unwrap(), "%m/%d/%Y")
							.unwrap()
							.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
					) + chrono::Duration::hours(4);

					let class = Class {
						section,
						component,
						time: time.into_datetime_range(start),
						location,
						address,
						instructor,

						end,
					};

					prev = Some(class.clone());
					class
				},
			)
			.collect::<Vec<_>>();

		courses.push(Course {
			name,
			code,
			status,
			classes,
		});
	}

	courses
}
