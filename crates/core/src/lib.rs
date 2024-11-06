#![feature(iter_array_chunks)]
#![warn(clippy::pedantic)]

use course::Course;
use icalendar::{Calendar, CalendarDateTime, Component, Event, EventLike};

pub mod course;

pub const TZ: chrono_tz::Tz = chrono_tz::America::Toronto;

#[must_use]
pub fn create_calendar(courses: Vec<Course>) -> Calendar {
	let mut calendar = Calendar::new();

	calendar.name("University of Ottawa");
	calendar.timezone(TZ.name());

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
					tzid: TZ.name().to_string(),
				})
				.ends(CalendarDateTime::WithTimezone {
					date_time: end.naive_local(),
					tzid: TZ.name().to_string(),
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

	calendar
}
