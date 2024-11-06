#![warn(clippy::pedantic)]

use std::{fs::File, io::Write, path::PathBuf};

use clap::Parser;

#[derive(Parser)]
struct Args {
	#[clap(value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
	path: Option<PathBuf>,
	#[clap(short, long, value_hint = clap::ValueHint::FilePath)]
	output: Option<PathBuf>,
}

fn main() {
	let args = Args::parse();
	let courses = uo2ics_core::course::parse_from_file(args.path);
	let calendar = uo2ics_core::create_calendar(courses);

	if let Some(output) = args.output {
		let mut file = File::create(output).unwrap();
		write!(&mut file, "{calendar}").unwrap();
	} else {
		write!(&mut std::io::stdout(), "{calendar}").unwrap();
	}
}
