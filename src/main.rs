#![feature(iter_array_chunks)]
#![warn(clippy::pedantic)]

mod course;

use std::{fs::File, io::Write, path::PathBuf};

use clap::Parser;
use icalendar::{Calendar, CalendarDateTime, Component, Event, EventLike};

#[derive(Parser)]
struct Args {
	#[clap(value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
	path: Option<PathBuf>,
	#[clap(short, long, value_hint = clap::ValueHint::FilePath)]
	output: Option<PathBuf>,
}

fn main() {
	let args = Args::parse();
	let tz = chrono_tz::America::Toronto;

	let courses = course::parse_from_file(args.path, tz);
	let mut calendar = Calendar::new();

	calendar.name("University of Ottawa");
	calendar.timezone(tz.name());

	for course in courses {
		if matches!(course.status, course::Status::Waiting) {
			continue;
		}

		for class in course.classes {
			let mut event = Event::new();

			event.summary(&format!(
				"{} ({}) {}",
				class.location, class.component, course.code
			));

			let start = class.time.start;
			let end = class.time.end;

			event
				.starts(CalendarDateTime::WithTimezone {
					date_time: start.naive_local(),
					tzid: tz.name().to_string(),
				})
				.ends(CalendarDateTime::WithTimezone {
					date_time: end.naive_local(),
					tzid: tz.name().to_string(),
				})
				.location(&format!("{}, Ottawa, ON, Canada", class.address))
				.description(&format!(
					"Name: {} | Section: {} | Instructor: {}",
					course.name, class.section, class.instructor
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
