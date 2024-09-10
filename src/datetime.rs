use chrono::{DateTime, Datelike, FixedOffset};
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

// Helper functions

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

    // Consolidate identical blocks
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

    // Combine the two identical blocks
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

    // Determine the suffix only if the word form doesn't already include it
    if word.ends_with("first") || word.ends_with("second") || word.ends_with("third") {
        return word;
    }

    // Determine the suffix based on the last digit
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
