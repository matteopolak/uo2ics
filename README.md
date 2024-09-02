# uo2ics

A simple script to convert University of Ottawa class schedules to iCalendar format.

## Usage

Go to your `My Class Schedule` page on uoZone with `List View` (in English), then save the page.
In a folder of the same name, find the `SA_LEARNER_SERVICES.html` file and run the script.

```bash
Usage: uo2ics [OPTIONS] [FILE]

Arguments:
  [FILE]

Options:
  -o, --output <OUTPUT>
  -h, --help             Print help
```

## Example

```bash
uo2ics SA_LEARNER_SERVICES.html -o schedule.ics
```
