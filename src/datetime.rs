use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use num_format::{Locale, ToFormattedString};

use crate::Error;

pub fn format_custom_date(date: &DateTime<FixedOffset>, picture: &str) -> Result<String, Error> {
    let mut formatted_string = String::new();
    let mut inside_brackets = false;
    let mut current_pattern = String::new();
    let mut i = 0;
    let chars: Vec<char> = picture.chars().collect();

    while i < chars.len() {
        let ch = chars[i];

        if ch == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            formatted_string.push('[');
            i += 2; // Skip both [[
            continue;
        }

        if ch == ']' && i + 1 < chars.len() && chars[i + 1] == ']' {
            formatted_string.push(']');
            i += 2; // Skip both ]]
            continue;
        }

        if ch == '[' {
            inside_brackets = true;
            current_pattern.clear(); // Start new pattern
            i += 1;
            continue;
        }

        if ch == ']' {
            inside_brackets = false;

            let trimmed_pattern = current_pattern
                .trim()
                .replace("\n", "")
                .replace("\t", "")
                .replace(" ", "");

            formatted_string.push_str(&handle_pattern(&trimmed_pattern, date)?);

            current_pattern.clear();
            i += 1;
            continue;
        }

        if inside_brackets {
            current_pattern.push(ch);
        } else {
            formatted_string.push(ch);
        }

        i += 1;
    }

    Ok(formatted_string)
}

fn handle_pattern(pattern: &str, date: &DateTime<FixedOffset>) -> Result<String, Error> {
    match pattern {
        "X0001" => Ok(date.iso_week().year().to_string()),
        "D#1,2" => Ok(format!("{:02}", date.day())),
        "M1,2" => Ok(format!("{:02}", date.month())),
        "Y,2" => Ok(date.format("%y").to_string()),
        "Y0001,2" | "Y0001" => Ok(date.format("%Y").to_string()),
        "Y0001,2-2" | "Y##01,2-2" => handle_year_last_two_digits(date),
        "Da" => Ok(map_day_to_letter(date.day())),
        "MA" => Ok(map_month_to_letter(date.month())),
        "W01" => Ok(date.format("%V").to_string()),
        "Y" => Ok(date.format("%Y").to_string()),    // Year
        "D#1" => Ok(date.format("%-d").to_string()), // Day without leading zero
        "M#1" => Ok(date.format("%-m").to_string()), // Month without leading zero
        "F1" => Ok(date.format("%u").to_string()),
        "Y9,999,*" => Ok(date.year().to_formatted_string(&Locale::en)),
        "D1" => Ok(date.format("%-d").to_string()),
        "d" => Ok(calculate_total_days_in_year(date)),
        "D01" => Ok(date.format("%d").to_string()),
        "Dwo" => Ok(format_day_in_words_with_ordinal(date.day())),
        "dwo" => Ok(format_day_in_words_with_ordinal(date.ordinal())),
        "W" => Ok(format!("{}", date.iso_week().week())),
        "w" => Ok(handle_week_of_month(date)),
        "xNn" => Ok(handle_xnn(date)),
        "M01" => Ok(date.format("%m").to_string()),
        "H01" => Ok(date.format("%H").to_string()),
        "h" => Ok(date.format("%-I").to_string()),
        "m" => Ok(date.format("%M").to_string()),
        "s" => Ok(date.format("%S").to_string()),
        "f001" => Ok(date.format("%3f").to_string()),
        "Z01:01t" | "Z01:01" | "Z0101t" => handle_timezone(date, pattern),
        "Z" => Ok(date.format("%:z").to_string()),
        "z" => Ok(format!("GMT{}", date.format("%:z"))),
        "Z0" => Ok(handle_trimmed_timezone(date)),
        "Z010101t" => {
            // Handle specific case where more than four digits were used erroneously
            Err(Error::D3134Error("Invalid timezone format".to_string()))
        }
        "m01" => Ok(date.format("%M").to_string()), // Minutes with leading zero
        "s01" => Ok(date.format("%S").to_string()), // Seconds with leading zero
        "F0" => Ok(date.format("%u").to_string()),  // ISO day of the week (1-7)
        "FNn" => Ok(date.format("%A").to_string()),
        "FNn,3-3" => Ok(date.format("%A").to_string()[..3].to_string()),
        "h#1" => Ok(date.format("%-I").to_string()),
        "P" => Ok(date.format("%p").to_string().to_lowercase()),
        "PN" => Ok(date.format("%p").to_string()),
        "Pn" => Ok(date.format("%p").to_string().to_lowercase()),
        "YI" => Ok(to_roman_numerals(date.year())),
        "Yi" => Ok(to_roman_numerals_lower(date.year())),
        "D1o" => Ok(format_day_with_ordinal(date.day())),
        "MNn" => Ok(date.format("%B").to_string()),
        "MNn,3-3" => Ok(date.format("%B").to_string()[..3].to_string()),
        "MN" => Ok(date.format("%B").to_string().to_uppercase()),
        "YN" => Err(Error::D3133Error("Invalid timezone format".to_string())),
        "Yw" => Ok(to_year_in_words(date.year())),
        "E" => Ok("ISO".to_string()),
        "F" => Ok(date.format("%A").to_string().to_lowercase()),
        "D" => Ok(date.format("%-d").to_string()),
        "M" => Ok(date.format("%-m").to_string()),
        "C" => Ok("ISO".to_string()),
        _ => Ok(format!("[{}]", pattern)), // Treat unrecognized pattern as literal
    }
}

pub fn parse_custom_format(timestamp_str: &str, picture: &str) -> Option<i64> {
    match picture {
        // Handle ISO 8601 dates (including with timezone offsets like "+0000")
        "" => {
            // Handle year-only input (e.g., "2018")
            if let Some(millis) = parse_year_only(timestamp_str) {
                return Some(millis);
            }
            // Handle date-only input (e.g., "2017-10-30")
            if let Some(millis) = parse_date_only(timestamp_str) {
                return Some(millis);
            }
            // Handle ISO 8601 formats with timezone offsets (e.g., "2018-02-01T09:42:13.123+0000")
            if let Some(millis) = parse_iso8601_with_timezone(timestamp_str) {
                return Some(millis);
            }
            // Handle other standard ISO 8601 formats (e.g., "1970-01-01T00:00:00.001Z")
            if let Some(millis) = parse_iso8601_date(timestamp_str) {
                return Some(millis);
            }
            None
        }

        // Handle the simple year format "[Y1]"
        "[Y1]" => {
            if let Ok(year) = timestamp_str.parse::<i32>() {
                let parsed_year = NaiveDate::from_ymd_opt(year, 1, 1)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_year, time);
                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle Roman numeral year format "[YI]" (e.g., 'MCMLXXXIV')
        "[YI]" => {
            if let Some(year) = roman_to_int(timestamp_str) {
                let parsed_year = NaiveDate::from_ymd_opt(year, 1, 1)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_year, time);
                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle the format '[Yw]' (e.g., 'one thousand, nine hundred and eighty-four')
        "[Yw]" => {
            // Convert the word-based year (e.g., 'one thousand, nine hundred and eighty-four') to a number
            let year = words_to_number(&timestamp_str.to_lowercase())?;

            // Set the date to January 1st of the given year
            let parsed_date = NaiveDate::from_ymd_opt(year, 1, 1)?;
            let time = NaiveTime::from_hms_opt(0, 0, 0)?;
            let datetime = NaiveDateTime::new(parsed_date, time);

            Some(Utc.from_utc_datetime(&datetime).timestamp_millis())
        }

        "[Y]-[M]-[D]" => {
            if let Some(millis) = parse_ymd_date(timestamp_str) {
                return Some(millis);
            }
            None
        }

        // Handle the format '[H]:[m]' (e.g., '13:45')
        "[H]:[m]" => {
            let parts: Vec<&str> = timestamp_str.split(':').collect();
            if parts.len() == 2 {
                // Parse the hour and minute
                let hour: u32 = parts[0].parse().ok()?;
                let minute: u32 = parts[1].parse().ok()?;

                // Use the current date along with the given time
                let now = Utc::now(); // Get current date
                let parsed_date = NaiveDate::from_ymd_opt(now.year(), now.month(), now.day())?;
                let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);

                // Return the timestamp in milliseconds
                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Custom date format handling with time and AM/PM
        "[D1]/[M1]/[Y0001] [h]:[m] [P]" => {
            if let Some(parsed_datetime) = parse_custom_date(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Some(utc_datetime.timestamp_millis());
            }
            None
        }

        // Handle the format '[Y0001]-[d001]' (e.g., '2018-094')
        "[Y0001]-[d001]" => {
            if let Some(parsed_datetime) = parse_ordinal_date(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Some(utc_datetime.timestamp_millis());
            }
            None
        }

        // Handle the format '[FNn], [D1o] [MNn] [Y]' (e.g., 'Wednesday, 14th November 2018')
        "[FNn], [D1o] [MNn] [Y]" => {
            if let Some(parsed_datetime) = parse_custom_date_with_weekday(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Some(utc_datetime.timestamp_millis());
            }
            None
        }

        // Handle the format '[FNn,*-3], [DWwo] [MNn] [Y]' (e.g., 'Mon, Twelfth November 2018')
        "[FNn,*-3], [DWwo] [MNn] [Y]" => {
            if let Some(parsed_datetime) = parse_custom_date_with_weekday_and_ordinal(timestamp_str)
            {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Some(utc_datetime.timestamp_millis());
            }
            None
        }

        // Handle the format '[dwo] day of [Y]' (e.g., 'three hundred and sixty-fifth day of 2018')
        "[dwo] day of [Y]" => {
            if let Some(parsed_datetime) = parse_ordinal_day_of_year(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Some(utc_datetime.timestamp_millis());
            }
            None
        }

        // Handle the format '[Y]--[d]' (e.g., '2018--180')
        "[Y]--[d]" => {
            if let Some(parsed_datetime) = parse_ordinal_date_with_dashes(timestamp_str) {
                let utc_datetime = Utc.from_utc_datetime(&parsed_datetime);
                return Some(utc_datetime.timestamp_millis());
            }
            None
        }

        // Handle the format '[Dw] [MNn] [Y0001]' (e.g., 'twenty-seven April 2008')
        "[Dw] [MNn] [Y0001]" => {
            // Split the timestamp string into parts (day, month, year)
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() != 3 {
                return None;
            }

            let day_str = remove_day_suffix(parts[0]);
            let day = words_to_number(&day_str.to_lowercase())? as u32;

            // Convert the month name (e.g., 'April') into its corresponding numeric value
            let month = month_name_to_int(parts[1])?;

            // Handle the year: try parsing it as a direct number first, then fall back to word-based parsing
            let year_str = parts[2..].join(" ");
            let year = match year_str.parse::<i32>() {
                Ok(num) => num,
                Err(_) => words_to_number(&year_str)? as i32, // If it's word-based (e.g., 'two thousand and seventeen')
            };
            println!("year {}", year);

            let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
            let time = NaiveTime::from_hms_opt(0, 0, 0)?;
            let datetime = NaiveDateTime::new(parsed_date, time);

            Some(Utc.from_utc_datetime(&datetime).timestamp_millis())
        }

        // Handle the format '[D1] [M01] [YI]' (e.g., '27 03 MMXVIII')
        "[D1] [M01] [YI]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = parts[0].parse().ok()?;
                let month: u32 = parts[1].parse().ok()?;
                let year = roman_to_int(parts[2])?;
                let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);
                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle the format '[D1] [Mi] [YI]' (e.g., '27 iii MMXVIII')
        "[D1] [Mi] [YI]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = parts[0].parse().ok()?;
                let month = roman_to_int(parts[1].to_uppercase().as_str())? as u32;
                let year = roman_to_int(parts[2])?;

                let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);

                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle the format '[Da] [MA] [Yi]' (e.g., 'w C mmxviii')
        "[Da] [MA] [Yi]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let month = roman_month_to_int(parts[1])?;
                let year = roman_to_int(parts[2].to_uppercase().as_str())?;
                let day = alphabetic_to_day(parts[0])?;

                let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);

                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle the format '[D1o] [M#1] [Y0001]' (e.g., '27th 3 1976')
        "[D1o] [M#1] [Y0001]" => {
            let cleaned_timestamp = timestamp_str
                .replace("th", "")
                .replace("st", "")
                .replace("nd", "")
                .replace("rd", "");
            let parts: Vec<&str> = cleaned_timestamp.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = parts[0].parse().ok()?;
                let month: u32 = parts[1].parse().ok()?;
                let year: i32 = parts[2].parse().ok()?;
                let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);
                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle the format '[D1o] [MNn] [Y0001]' (e.g., '27th April 2008')
        "[D1o] [MNn] [Y0001]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day = remove_ordinal_suffix(parts[0])?;
                let month = month_name_to_int(parts[1])?;
                let year = parts[2].parse::<i32>().ok()?;

                let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);

                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle the format '[D1] [MNn] [Y0001]' (e.g., '21 August 2017')
        "[D1] [MNn] [Y0001]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day = parts[0].parse::<u32>().ok()?;
                let month = month_name_to_int(parts[1])?;
                let year = parts[2].parse::<i32>().ok()?;

                let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);

                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle the format '[D1] [MNn,3-3] [Y0001]' (e.g., '2 Feb 2012')
        "[D1] [MNn,3-3] [Y0001]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() == 3 {
                let day = parts[0].parse::<u32>().ok()?;
                let month = abbreviated_month_to_int(parts[1])?;
                let year = parts[2].parse::<i32>().ok()?;

                let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);

                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle the format '[D1o] [M01] [Y0001]' (e.g., '21st 12 1881')
        "[D1o] [M01] [Y0001]" => {
            let cleaned_timestamp = timestamp_str
                .replace("th", "")
                .replace("st", "")
                .replace("nd", "")
                .replace("rd", "");
            let parts: Vec<&str> = cleaned_timestamp.split_whitespace().collect();
            if parts.len() == 3 {
                let day: u32 = parts[0].parse().ok()?;
                let month: u32 = parts[1].parse().ok()?;
                let year: i32 = parts[2].parse().ok()?;
                let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);
                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle ISO 8601-like formats with custom pattern handling
        "[Y0001]-[M01]-[D01]" => {
            if let Ok(parsed_date) = NaiveDate::parse_from_str(timestamp_str, "%Y-%m-%d") {
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);
                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        // Handle ISO 8601-like formats with custom pattern handling like '[Y1]-[M01]-[D01]'
        "[Y1]-[M01]-[D01]" => {
            if let Ok(parsed_date) = NaiveDate::parse_from_str(timestamp_str, "%Y-%m-%d") {
                let time = NaiveTime::from_hms_opt(0, 0, 0)?;
                let datetime = NaiveDateTime::new(parsed_date, time);
                return Some(Utc.from_utc_datetime(&datetime).timestamp_millis());
            }
            None
        }

        "[Y0001]-[M01]-[D01]T[H01]:[m01]:[s01].[f001]Z" => {
            if let Ok(parsed_datetime) = DateTime::parse_from_rfc3339(timestamp_str) {
                return Some(parsed_datetime.timestamp_millis());
            }
            None
        }

        // Handle the format '[Dw] [MNn] [Yw]' (e.g., 'twenty-first August two thousand and seventeen')
        "[Dw] [MNn] [Yw]" => {
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() < 5 {
                return None;
            }

            let day_str = parse_day_str(parts[0]);
            let day = words_to_number(&day_str.to_lowercase())? as u32;
            let month = month_name_to_int(parts[1])?;

            let year_str = parts[2..].join(" ");
            let year = words_to_number(&year_str.to_lowercase())? as i32;

            let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
            let time = NaiveTime::from_hms_opt(0, 0, 0)?;
            let datetime = NaiveDateTime::new(parsed_date, time);

            // Return the timestamp in milliseconds
            Some(Utc.from_utc_datetime(&datetime).timestamp_millis())
        }

        "[DW] [MNn] [Yw]" => {
            // Split the timestamp string into parts (day, month, year)
            let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
            if parts.len() < 5 {
                return None;
            }

            let day_str = parse_day_str(parts[0]);
            let day = words_to_number(&day_str.to_lowercase())? as u32;
            let month = month_name_to_int(parts[1])?;

            let year_str = parts[2..].join(" ");
            let year = words_to_number(&year_str.to_lowercase())? as i32;

            let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
            let time = NaiveTime::from_hms_opt(0, 0, 0)?;
            let datetime = NaiveDateTime::new(parsed_date, time);

            // Return the timestamp in milliseconds
            Some(Utc.from_utc_datetime(&datetime).timestamp_millis())
        }

        // Handle the format '[DW] of [MNn], [Yw]' (e.g., 'Twentieth of August, two thousand and seventeen')
        "[DW] of [MNn], [Yw]" => {
            let cleaned_str = timestamp_str.replace("of", "").replace(",", "");
            let parts: Vec<&str> = cleaned_str.split_whitespace().collect();
            if parts.len() < 5 {
                return None;
            }

            let day_str = parts[0]; // Handle the day part (e.g., "Twentieth")
            let day = words_to_number(&day_str.to_lowercase())? as u32;
            let month = month_name_to_int(parts[1])?; // Handle the month (e.g., "August")

            let year_str = parts[2..].join(" ");
            let year = words_to_number(&year_str.to_lowercase())? as i32;

            let parsed_date = NaiveDate::from_ymd_opt(year, month, day)?;
            let time = NaiveTime::from_hms_opt(0, 0, 0)?;
            let datetime = NaiveDateTime::new(parsed_date, time);

            Some(Utc.from_utc_datetime(&datetime).timestamp_millis())
        }

        // Default case: return None if the picture is not recognized
        _ => None,
    }
}

fn parse_day_str(day_str: &str) -> String {
    // Split the day string on hyphen, convert to lowercase, and join it back
    day_str
        .split('-')
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

fn handle_year_last_two_digits(date: &DateTime<FixedOffset>) -> Result<String, Error> {
    let year = date.year();
    let last_two_digits = year % 100; // Extract last 2 digits of the year
    Ok(format!("{:02}", last_two_digits))
}

fn map_day_to_letter(day: u32) -> String {
    match day {
        1..=26 => (b'a' + (day - 1) as u8) as char,
        27..=31 => (b'a' + (day - 27) as u8) as char,
        _ => ' ',
    }
    .to_string()
}

fn map_month_to_letter(month: u32) -> String {
    match month {
        1..=12 => (b'a' + (month - 1) as u8) as char,
        _ => ' ',
    }
    .to_uppercase()
    .to_string()
}

fn calculate_total_days_in_year(date: &DateTime<FixedOffset>) -> String {
    let total_days = if date.year() % 4 == 0 && (date.year() % 100 != 0 || date.year() % 400 == 0) {
        366 // Leap year
    } else {
        365 // Regular year
    };
    total_days.to_string()
}

fn handle_week_of_month(date: &DateTime<FixedOffset>) -> String {
    let iso_week = date.iso_week().week();
    let month = date.month();
    let day_of_month = date.day();
    let first_day_of_month = date.with_day(1).unwrap();
    let first_weekday_of_month = first_day_of_month.weekday().num_days_from_sunday();
    let week_of_month = ((day_of_month + first_weekday_of_month - 1) / 7) + 1;

    if (month == 12 && iso_week == 1)
        || (week_of_month == 5 && month == 1 && iso_week == 5)
        || (week_of_month == 1 && first_weekday_of_month == 5 && iso_week == 5)
    {
        format!("{}", iso_week)
    } else if week_of_month == 5 && first_weekday_of_month == 0 {
        format!("{}", 1)
    } else if month == 1 && iso_week >= 52 && first_weekday_of_month == 0 {
        format!("{}", 5)
    } else {
        format!("{}", week_of_month)
    }
}

fn handle_trimmed_timezone(date: &DateTime<FixedOffset>) -> String {
    let tz_offset = date.format("%z").to_string();

    if tz_offset == "+0000" || tz_offset == "-0000" {
        "0".to_string()
    } else if tz_offset[3..] == *"00" {
        format!(
            "{}{}",
            &tz_offset[..1],
            tz_offset[1..3].trim_start_matches('0')
        )
    } else {
        format!(
            "{}{}:{}",
            &tz_offset[..1],
            tz_offset[1..3].trim_start_matches('0'),
            &tz_offset[3..]
        )
    }
}

fn handle_xnn(date: &DateTime<FixedOffset>) -> String {
    let days_from_monday = date.weekday().num_days_from_monday() as i64;
    let first_day_of_week = *date - chrono::Duration::days(days_from_monday);
    let last_day_of_week = first_day_of_week + chrono::Duration::days(6);
    let first_day_month = first_day_of_week.month();
    let last_day_month = last_day_of_week.month();

    let week_month = if first_day_month != last_day_month {
        if last_day_of_week.day() >= 4 {
            last_day_month
        } else {
            first_day_month
        }
    } else {
        first_day_month
    };

    chrono::NaiveDate::from_ymd_opt(date.year(), week_month, 1)
        .expect("Invalid month or day")
        .format("%B")
        .to_string()
}

fn handle_timezone(date: &DateTime<FixedOffset>, pattern: &str) -> Result<String, Error> {
    match pattern {
        "Z01:01t" => {
            if date.offset().local_minus_utc() == 0 {
                Ok("Z".to_string()) // UTC, add 'Z'
            } else {
                Ok(date.format("%:z").to_string()) // Timezone offset with colon (e.g., "+01:00")
            }
        }
        "Z01:01" => {
            if date.offset().local_minus_utc() == 0 {
                Ok("+00:00".to_string()) // UTC, output '+00:00' instead of 'Z'
            } else {
                let offset_minutes = date.offset().local_minus_utc() / 60;
                let hours = offset_minutes / 60;
                let minutes = offset_minutes % 60;
                Ok(format!("{:+03}:{:02}", hours, minutes)) // Format as '+01:00' or '-05:00'
            }
        }
        "Z0101t" => {
            if date.offset().local_minus_utc() == 0 {
                Ok("Z".to_string()) // UTC, add 'Z'
            } else {
                let offset_minutes = date.offset().local_minus_utc() / 60;
                let hours = offset_minutes / 60;
                let minutes = offset_minutes % 60;
                Ok(format!("{:+03}{:02}", hours, minutes)) // Format as '+0100' or '-0500' without colon
            }
        }
        _ => Err(Error::D3134Error("Invalid timezone format".to_string())),
    }
}

pub fn format_day_with_ordinal(day: u32) -> String {
    match day {
        1 | 21 | 31 => format!("{}st", day),
        2 | 22 => format!("{}nd", day),
        3 | 23 => format!("{}rd", day),
        _ => format!("{}th", day),
    }
}

fn to_year_in_words(year: i32) -> String {
    if year < 0 {
        return format!("minus {}", to_year_in_words(-year));
    }

    let below_20 = [
        "",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    let tens = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];

    let mut result = String::new();
    let mut y = year;

    // Handle thousands (e.g., 2000 -> "two thousand")
    if y >= 1000 {
        let thousands = y / 1000;
        result.push_str(below_20[thousands as usize]);
        result.push_str(" thousand");
        y %= 1000;

        // Add "and" only if the number is non-zero and below 1000
        if y > 0 && y < 100 {
            result.push_str(" and ");
        } else if y > 0 {
            result.push(' ');
        }
    }

    // Handle hundreds (e.g., 800 -> "eight hundred")
    if y >= 100 {
        let hundreds = y / 100;
        result.push_str(below_20[hundreds as usize]);
        result.push_str(" hundred");
        y %= 100;

        if y > 0 {
            result.push_str(" and ");
        }
    }

    // Handle tens and ones
    if y >= 20 {
        let t = y / 10;
        result.push_str(tens[t as usize]);
        y %= 10;

        if y > 0 {
            result.push('-');
        }
    }

    if y > 0 {
        result.push_str(below_20[y as usize]);
    }

    result.trim().to_string()
}

pub fn to_roman_numerals(year: i32) -> String {
    let mut year = year;
    let mut roman = String::new();
    let numerals = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    for &(value, symbol) in &numerals {
        while year >= value {
            roman.push_str(symbol);
            year -= value;
        }
    }
    roman
}

pub fn to_roman_numerals_lower(year: i32) -> String {
    to_roman_numerals(year).to_lowercase()
}

// Helper function to parse timezone strings like "±HHMM"
pub fn parse_timezone_offset(timezone: &str) -> Option<FixedOffset> {
    if timezone == "0000" {
        return FixedOffset::east_opt(0); // UTC
    }
    if timezone.len() != 5 {
        return None;
    }

    let (hours, minutes) = (
        timezone[1..3].parse::<i32>().ok()?,
        timezone[3..5].parse::<i32>().ok()?,
    );
    let total_offset_seconds = (hours * 3600) + (minutes * 60);

    match &timezone[0..1] {
        "+" => FixedOffset::east_opt(total_offset_seconds),
        "-" => FixedOffset::west_opt(total_offset_seconds),
        _ => None,
    }
}

fn format_day_in_words_with_ordinal(day: u32) -> String {
    let word = to_words(day); // Convert the number to words

    // Special cases for 11th, 12th, and 13th
    if (11..=13).contains(&(day % 100)) {
        return format!("{}th", word);
    }

    if word.ends_with("first") || word.ends_with("second") || word.ends_with("third") {
        return word;
    }

    let suffix = match day % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };

    format!("{}{}", word, suffix)
}

fn to_words(num: u32) -> String {
    let below_20 = [
        "",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
    ];
    let tens = [
        "",
        "",
        "twentieth",
        "thirtieth",
        "fortieth",
        "fiftieth",
        "sixtieth",
        "seventieth",
        "eightieth",
        "ninetieth",
    ];
    let tens_with_units = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];

    // Handle numbers below 20
    if num < 20 {
        below_20[num as usize].to_string()
    } else if num < 100 {
        // Handle multiples of 10 (20, 30, etc.)
        if num % 10 == 0 {
            return tens[(num / 10) as usize].to_string();
        }
        // Handle numbers between 21-99
        let ten = tens_with_units[(num / 10) as usize];
        let unit = below_20[(num % 10) as usize];
        format!("{}-{}", ten, unit)
    } else {
        num.to_string()
    }
}

fn roman_to_int(s: &str) -> Option<i32> {
    let mut total = 0;
    let mut prev_value = 0;

    for c in s.chars().rev() {
        let value = match c {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None, // Invalid Roman numeral
        };

        if value < prev_value {
            total -= value;
        } else {
            total += value;
        }

        prev_value = value;
    }

    Some(total)
}

fn roman_month_to_int(month_str: &str) -> Option<u32> {
    match month_str.to_uppercase().as_str() {
        "I" => Some(1),    // January
        "II" => Some(2),   // February
        "III" => Some(3),  // March
        "IV" => Some(4),   // April
        "V" => Some(5),    // May
        "VI" => Some(6),   // June
        "VII" => Some(7),  // July
        "VIII" => Some(8), // August
        "IX" => Some(9),   // September
        "X" => Some(10),   // October
        "XI" => Some(11),  // November
        "XII" => Some(12), // December
        "C" => Some(3),    // Fix for 'C' (March)
        _ => None,         // Unsupported month
    }
}

fn alphabetic_to_day(s: &str) -> Option<u32> {
    let chars: Vec<char> = s.chars().collect();

    if chars.len() == 1 {
        // Single-letter day (e.g., 'w' -> 23)
        let day = chars[0].to_ascii_lowercase() as u32 - 'a' as u32 + 1;
        return if day <= 31 { Some(day) } else { None };
    } else if chars.len() == 2 {
        // Two-letter day (e.g., 'ae')
        let first = chars[0].to_ascii_lowercase() as u32 - 'a' as u32 + 1;
        let second = chars[1].to_ascii_lowercase() as u32 - 'a' as u32 + 1;

        // Base-26 calculation for two-letter day
        let day = first * 26 + second;
        // Ensure the day is valid (1 to 31)
        return if day <= 31 { Some(day) } else { None };
    }

    None // Invalid day format
}

fn remove_day_suffix(day_str: &str) -> String {
    // Remove specific ordinal suffixes ('st', 'nd', 'rd', 'th') only if they appear at the end of the string
    if day_str.ends_with("st") {
        day_str.trim_end_matches("st").to_string()
    } else if day_str.ends_with("nd") {
        day_str.trim_end_matches("nd").to_string()
    } else if day_str.ends_with("rd") {
        day_str.trim_end_matches("rd").to_string()
    } else if day_str.ends_with("th") {
        day_str.trim_end_matches("th").to_string()
    } else {
        day_str.to_string() // Return the original string if no ordinal suffix is present
    }
}

fn remove_ordinal_suffix(day_str: &str) -> Option<u32> {
    // Remove suffixes like 'st', 'nd', 'rd', 'th' from the day string
    let cleaned_day = day_str.trim_end_matches(|c: char| c.is_alphabetic());
    cleaned_day.parse::<u32>().ok()
}

fn month_name_to_int(month_str: &str) -> Option<u32> {
    match month_str.to_lowercase().as_str() {
        "january" => Some(1),
        "february" => Some(2),
        "march" => Some(3),
        "april" => Some(4),
        "may" => Some(5),
        "june" => Some(6),
        "july" => Some(7),
        "august" => Some(8),
        "september" => Some(9),
        "october" => Some(10),
        "november" => Some(11),
        "december" => Some(12),
        _ => None,
    }
}

fn abbreviated_month_to_int(month_str: &str) -> Option<u32> {
    match month_str.to_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

// Split the word string into tokens, handling hyphenated and punctuated numbers
fn words_to_number(word_str: &str) -> Option<i32> {
    let units = [
        ("zero", 0),
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
        ("thirteen", 13),
        ("fourteen", 14),
        ("fifteen", 15),
        ("sixteen", 16),
        ("seventeen", 17),
        ("eighteen", 18),
        ("nineteen", 19),
        // Ordinal units
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("sixth", 6),
        ("seventh", 7),
        ("eighth", 8),
        ("ninth", 9),
        ("tenth", 10),
        ("eleventh", 11),
        ("twelfth", 12),
        ("thirteenth", 13),
        ("fourteenth", 14),
        ("fifteenth", 15),
        ("sixteenth", 16),
        ("seventeenth", 17),
        ("eighteenth", 18),
        ("nineteenth", 19),
        ("twentieth", 20),
        ("twenty-first", 21),
        ("twenty-second", 22),
        ("twenty-third", 23),
        ("twenty-fourth", 24),
        ("twenty-fifth", 25),
        ("twenty-sixth", 26),
        ("twenty-seventh", 27),
        ("twenty-eighth", 28),
        ("twenty-ninth", 29),
        ("thirtieth", 30),
        ("thirty-first", 31),
    ];

    let tens = [
        ("twenty", 20),
        ("thirty", 30),
        ("forty", 40),
        ("fifty", 50),
        ("sixty", 60),
        ("seventy", 70),
        ("eighty", 80),
        ("ninety", 90),
    ];

    let scales = [("hundred", 100), ("thousand", 1000)];

    let mut result = 0;
    let mut current = 0;
    let mut last_ten = None;

    // Split the word string into tokens, handling hyphenated and punctuated numbers
    for word in word_str
        .replace(",", "")
        .to_lowercase() // Convert the input to lowercase
        .split_whitespace()
        .flat_map(|w| w.split('-'))
    {
        // Skip "and"
        if word == "and" {
            continue;
        }

        // Check if it's a unit or ordinal unit
        if let Some(unit) = units.iter().find(|&&(w, _)| w == word).map(|(_, n)| n) {
            if let Some(ten) = last_ten {
                current += ten + unit;
                last_ten = None;
            } else {
                current += unit;
            }
        }
        // Check if it's a ten or ordinal ten
        else if let Some(ten) = tens.iter().find(|&&(w, _)| w == word).map(|(_, n)| n) {
            if let Some(ten_value) = last_ten {
                current += ten_value + ten;
            } else {
                last_ten = Some(ten);
            }
        }
        // Check if it's a scale (e.g., "hundred", "thousand")
        else if let Some(scale) = scales.iter().find(|&&(w, _)| w == word).map(|(_, n)| n) {
            if *scale == 100 {
                current *= scale; // multiply current value (e.g., "two hundred")
            } else if *scale == 1000 {
                // For "thousand", we add to the result and accumulate the rest
                result += current * scale; // e.g., "two thousand" -> 2000
                current = 0; // Reset to accumulate further values (like "seventeen")
            }
        }
    }

    result += current; // Add the remaining value (e.g., "seventeen")
    Some(result)
}

fn parse_custom_date(date_str: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = date_str.split_whitespace().collect();

    if parts.len() != 3 {
        return None; // Ensure we have the correct number of parts (date, time, AM/PM)
    }

    // Split the date part into day, month, and year
    let date_parts: Vec<&str> = parts[0].split('/').collect();
    if date_parts.len() != 3 {
        return None; // Ensure we have day, month, year in the correct format
    }

    let day: u32 = date_parts[0].parse().ok()?;
    let month: u32 = date_parts[1].parse().ok()?;
    let year: i32 = date_parts[2].parse().ok()?;

    // Split the time part into hours and minutes
    let time_parts: Vec<&str> = parts[1].split(':').collect();
    if time_parts.len() != 2 {
        return None; // Ensure we have hour and minute in the correct format
    }

    let mut hour: u32 = time_parts[0].parse().ok()?;
    let minute: u32 = time_parts[1].parse().ok()?;

    // Parse the AM/PM part and adjust the hour accordingly
    let am_pm = parts[2].to_lowercase();
    if am_pm == "am" {
        if hour == 12 {
            hour = 0; // 12 AM is midnight, hour 0
        }
    } else if am_pm == "pm" {
        if hour != 12 {
            hour += 12; // PM shifts the hour to 12-hour format
        }
    } else {
        return None; // Invalid AM/PM part
    }

    // Construct NaiveDateTime from the parsed date and time parts
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    Some(NaiveDateTime::new(date, time))
}

fn parse_ordinal_date(date_str: &str) -> Option<NaiveDateTime> {
    // Split the input based on the '-' separator
    let parts: Vec<&str> = date_str.split('-').collect();

    // Ensure we have exactly two parts: the year and the day of the year
    if parts.len() != 2 {
        return None;
    }

    // Parse the year (e.g., "2018")
    let year: i32 = parts[0].parse().ok()?;

    // Parse the day of the year (ordinal date, e.g., "094")
    let ordinal_day: u32 = parts[1].parse().ok()?;

    // Use from_yo_opt to create the date from the year and ordinal day
    let date = NaiveDate::from_yo_opt(year, ordinal_day)?;

    // Use 00:00:00 for the time portion
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;

    Some(NaiveDateTime::new(date, time))
}

fn parse_custom_date_with_weekday(date_str: &str) -> Option<NaiveDateTime> {
    // Example input: "Wednesday, 14th November 2018"
    let parts: Vec<&str> = date_str
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|&x| !x.is_empty())
        .collect();

    // Ensure we have 4 parts: "Wednesday", "14th", "November", "2018"
    if parts.len() != 4 {
        return None;
    }

    // Parse the ordinal day (e.g., "14th" -> 14)
    let day_str = parts[1].trim_end_matches(|c: char| c.is_alphabetic());
    let day: u32 = day_str.parse().ok()?;

    // Parse the full month name (e.g., "November")
    let month = match parts[2].to_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        _ => return None, // Invalid month name
    };

    // Parse the year (e.g., "2018")
    let year: i32 = parts[3].parse().ok()?;

    // Construct the NaiveDateTime from the parsed values
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;
    Some(NaiveDateTime::new(date, time))
}

fn parse_custom_date_with_weekday_and_ordinal(date_str: &str) -> Option<NaiveDateTime> {
    // Example input: "Mon, Twelfth November 2018"
    let parts: Vec<&str> = date_str
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|&x| !x.is_empty())
        .collect();

    // Ensure we have 4 parts: "Mon", "Twelfth", "November", "2018"
    if parts.len() != 4 {
        return None;
    }

    // Parse the ordinal day in words (e.g., "Twelfth")
    let day = words_to_number(parts[1])? as u32;

    // Parse the full month name (e.g., "November")
    let month = match parts[2].to_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        _ => return None, // Invalid month name
    };

    // Parse the year (e.g., "2018")
    let year: i32 = parts[3].parse().ok()?;

    // Construct the NaiveDateTime from the parsed values
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;
    Some(NaiveDateTime::new(date, time))
}

fn parse_ordinal_day_of_year(date_str: &str) -> Option<NaiveDateTime> {
    // Example input: "three hundred and sixty-fifth day of 2018"
    let parts: Vec<&str> = date_str.split_whitespace().collect();

    // The input should follow the format: "[ordinal] day of [year]"
    if parts.len() < 5 {
        return None;
    }

    // Combine the first 3+ parts into the ordinal word (e.g., "three hundred and sixty-fifth")
    let ordinal_day_words = parts[..(parts.len() - 3)].join(" ");

    // Convert the ordinal day in words to a number
    let day_of_year = words_to_number(&ordinal_day_words)? as u32;

    // Parse the year (e.g., "2018")
    let year: i32 = parts.last()?.parse().ok()?;

    // Construct the NaiveDate using the day of the year
    let parsed_date = NaiveDate::from_yo_opt(year, day_of_year)?;
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;
    Some(NaiveDateTime::new(parsed_date, time))
}

fn parse_ordinal_date_with_dashes(date_str: &str) -> Option<NaiveDateTime> {
    // Example input: "2018--180"
    let parts: Vec<&str> = date_str.split("--").collect();

    // Ensure we have exactly two parts: the year and the day of the year
    if parts.len() != 2 {
        return None;
    }

    // Parse the year (e.g., "2018")
    let year: i32 = parts[0].parse().ok()?;

    // Parse the ordinal day of the year (e.g., "180")
    let ordinal_day: u32 = parts[1].parse().ok()?;

    // Use from_yo_opt to create the date from the year and ordinal day
    let date = NaiveDate::from_yo_opt(year, ordinal_day)?;

    // Use 00:00:00 for the time portion
    let time = NaiveTime::from_hms_opt(0, 0, 0)?;

    // Combine the date and time into NaiveDateTime
    Some(NaiveDateTime::new(date, time))
}

fn parse_iso8601_date(date_str: &str) -> Option<i64> {
    // Attempt to parse the ISO 8601 formatted string
    if let Ok(datetime) = DateTime::parse_from_rfc3339(date_str) {
        // Convert the parsed datetime to milliseconds since the Unix Epoch
        Some(datetime.timestamp_millis())
    } else {
        None
    }
}

fn parse_iso8601_with_timezone(date_str: &str) -> Option<i64> {
    // Normalize the timezone format from "+0000" to "+00:00"
    let normalized_str = if date_str.ends_with("+0000") {
        date_str.replace("+0000", "+00:00")
    } else {
        date_str.to_string()
    };

    // Attempt to parse the normalized ISO 8601 formatted string
    if let Ok(datetime) = DateTime::parse_from_rfc3339(&normalized_str) {
        // Convert the parsed datetime to milliseconds since the Unix Epoch
        Some(datetime.timestamp_millis())
    } else {
        None
    }
}

fn parse_date_only(date_str: &str) -> Option<i64> {
    if let Ok(naive_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        if let Some(naive_datetime) = naive_date.and_hms_opt(0, 0, 0) {
            return Some(Utc.from_utc_datetime(&naive_datetime).timestamp_millis());
        }
    }
    None
}

fn parse_year_only(date_str: &str) -> Option<i64> {
    // Try parsing the string as a year (e.g., "2018")
    if let Ok(year) = date_str.parse::<i32>() {
        // Create a NaiveDate for January 1st of the given year
        if let Some(naive_date) = NaiveDate::from_ymd_opt(year, 1, 1) {
            // Set the time to 00:00:00
            if let Some(naive_datetime) = naive_date.and_hms_opt(0, 0, 0) {
                // Convert the NaiveDateTime to a UTC timestamp in milliseconds
                return Some(Utc.from_utc_datetime(&naive_datetime).timestamp_millis());
            }
        }
    }
    None
}

fn parse_ymd_date(timestamp_str: &str) -> Option<i64> {
    // Split the input timestamp by the '-' separator
    let parts: Vec<&str> = timestamp_str.split('-').collect();
    if parts.len() != 3 {
        return None; // Ensure the date format has exactly 3 parts (year, month, day)
    }

    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;

    if let Some(naive_date) = NaiveDate::from_ymd_opt(year, month, day) {
        // Set the time to 00:00:00 (midnight)
        if let Some(naive_datetime) = naive_date.and_hms_opt(0, 0, 0) {
            // Convert the NaiveDateTime to a UTC timestamp in milliseconds
            return Some(Utc.from_utc_datetime(&naive_datetime).timestamp_millis());
        }
    }
    None
}
