#![feature(iter_array_chunks)]
#![warn(clippy::pedantic)]

mod course;

use std::{fs::File, io::Write, path::PathBuf};

use chrono_tz::Tz;
use clap::Parser;
use icalendar::{Calendar, Component, Event, EventLike};

pub const TZ: Tz = chrono_tz::Canada::Eastern;

#[derive(Parser)]
struct Args {
	#[clap(value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
	path: Option<PathBuf>,
	#[clap(short, long, value_hint = clap::ValueHint::FilePath)]
	output: Option<PathBuf>,
}

fn main() {
	let args = Args::parse();

	let courses = course::parse_from_file(args.path);
	let mut calendar = Calendar::new();

	calendar.name("University of Ottawa");

	for course in courses {
		if matches!(course.status, course::Status::Waiting) {
			continue;
		}

		for class in course.classes {
			let mut event = Event::new();

			event.summary(&format!(
				"{} ({}) {}",
				class.location, class.component, course.name
			));

			let start = class.time.start;
			let end = class.time.end;

			event
				.starts(start.to_utc())
				.ends(end.to_utc())
				.location(&format!("{}, Ottawa, ON, Canada", class.address))
				.description(&format!(
					"Section: {} | Instructor: {}",
					class.section, class.instructor
				))
				// repeat weekly
				.add_property(
					"RRULE",
					rrule::RRule::new(rrule::Frequency::Weekly)
						.until(
							class
								.end
								.with_timezone(&rrule::Tz::Tz(class.end.timezone())),
						)
						.to_string(),
				)
				// add a reminder 30 minutes before
				.add_property(
					"VALARM",
					"TRIGGER:-PT30M;ACTION=DISPLAY;DESCRIPTION=Reminder",
				);

			calendar.push(event);
		}
	}

	if let Some(output) = args.output {
		let mut file = File::create(output).unwrap();
		write!(&mut file, "{calendar}").unwrap();
	} else {
		write!(&mut std::io::stdout(), "{calendar}").unwrap();
	}
}
