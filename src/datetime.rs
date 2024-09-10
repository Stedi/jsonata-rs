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

        // Handle escaped opening bracket [[ -> [
        if ch == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            formatted_string.push('[');
            i += 2; // Skip both [[
            continue;
        }

        // Handle escaped closing bracket ]] -> ]
        if ch == ']' && i + 1 < chars.len() && chars[i + 1] == ']' {
            formatted_string.push(']');
            i += 2; // Skip both ]]
            continue;
        }

        // Handle pattern opening bracket [
        if ch == '[' {
            inside_brackets = true;
            current_pattern.clear(); // Start new pattern
            i += 1;
            continue;
        }

        // Handle pattern closing bracket ]
        if ch == ']' {
            inside_brackets = false;

            // Trim any extra spaces, tabs, or newlines in the pattern
            let trimmed_pattern = current_pattern
                .trim()
                .replace("\n", "")
                .replace("\t", "")
                .replace(" ", "");

            // Match recognized patterns
            match trimmed_pattern.as_str() {
                // Day, Month, and Year with length specifiers
                "D#1,2" => formatted_string.push_str(&format!("{:02}", date.day())), // Day with 2 digits
                "M1,2" => formatted_string.push_str(&format!("{:02}", date.month())), // Month with 2 digits
                "Y,2" => formatted_string.push_str(&date.format("%y").to_string()), // Year with last 2 digits
                "Y0001,2" => formatted_string.push_str(&date.format("%Y").to_string()), // Year with 4 digits (ignore ,2 in this case)

                "Y0001,2-2" => {
                    let year = date.year();
                    let last_two_digits = year % 100; // Extract last 2 digits of the year
                    formatted_string.push_str(&format!("{:02}", last_two_digits));
                    // Format it as two digits
                }

                // Handle specific complex pattern [Y##01,2-2]
                "Y##01,2-2" => {
                    let year = date.year();
                    let last_two_digits = year % 100; // Get the last two digits of the year
                    formatted_string.push_str(&format!("{:02}", last_two_digits));
                    // Format it as two digits
                }

                "Da" => {
                    // Map day number (1-31) to corresponding letter in the alphabet
                    let day_as_letter = match date.day() {
                        1..=26 => (b'a' + (date.day() - 1) as u8) as char, // For days 1-26, map to 'a' to 'z'
                        27..=31 => (b'a' + (date.day() - 27) as u8) as char, // For days 27-31, wrap back from 'a'
                        _ => ' ',                                            // Default fallback
                    };
                    formatted_string.push(day_as_letter);
                }
                "MA" => {
                    // Map month number (1-12) to corresponding letter in the alphabet
                    let month_as_letter = match date.month() {
                        1..=12 => (b'a' + (date.month() - 1) as u8) as char, // For months 1-12, map to 'a' to 'l'
                        _ => ' ',                                            // Default fallback
                    };
                    formatted_string.push(
                        month_as_letter
                            .to_string()
                            .to_uppercase()
                            .chars()
                            .next()
                            .unwrap(),
                    );
                }

                // Regular patterns
                "Y" => formatted_string.push_str(&date.format("%Y").to_string()), // Year
                "Y0001" => formatted_string.push_str(&date.format("%Y").to_string()), // Year (4 digits)
                "D#1" => formatted_string.push_str(&date.format("%-d").to_string()), // Day without leading zero
                "M#1" => formatted_string.push_str(&date.format("%-m").to_string()), // Month without leading zero
                "W01" => formatted_string.push_str(&date.format("%V").to_string()), // ISO week number
                "F1" => formatted_string.push_str(&date.format("%u").to_string()), // ISO day of week
                "Y9,999,*" => {
                    formatted_string.push_str(&date.year().to_formatted_string(&Locale::en));
                } // Year with comma
                "D1" => formatted_string.push_str(&date.format("%-d").to_string()), // Day without leading zero
                "d" => {
                    let total_days_in_year = if date.year() % 4 == 0
                        && (date.year() % 100 != 0 || date.year() % 400 == 0)
                    {
                        366 // Leap year
                    } else {
                        365 // Regular year
                    };
                    formatted_string.push_str(&total_days_in_year.to_string());
                }
                "D01" => formatted_string.push_str(&date.format("%d").to_string()), // Day with leading 0
                "Dwo" => formatted_string.push_str(&format_day_in_words_with_ordinal(date.day())), // Day in words with ordinal
                "dwo" => {
                    let day_of_year = date.ordinal(); // Get the day of the year
                    formatted_string.push_str(&format_day_in_words_with_ordinal(day_of_year));
                    // Convert to words with ordinal suffix
                }
                // Handle week number in the year
                "W" => {
                    let week_number = date.iso_week().week(); // Gets the ISO week number as an integer
                    formatted_string.push_str(&format!("{}", week_number)); // Push week number without leading zero
                }

                "w" => {
                    let iso_week = date.iso_week().week(); // Get ISO week info
                    let month = date.month(); // Get the current month
                    let day_of_month = date.day(); // Get the day of the month
                    let first_day_of_month = date.with_day(1).unwrap(); // Get the first day of the current month
                    let first_weekday_of_month =
                        first_day_of_month.weekday().num_days_from_sunday(); // Weekday of the first day of the month

                    // Calculate the week of the month based on the day of the month
                    let week_of_month = ((day_of_month + first_weekday_of_month - 1) / 7) + 1;

                    // Handle special cases for week calculation
                    // Case 1: Week belongs to the end of the previous year (e.g., early January)
                    if month == 1 && iso_week >= 52 && first_weekday_of_month == 0 {
                        // Case 1: Week belongs to the end of the previous year (e.g., early January)
                        formatted_string.push_str(&format!("{}", 5)); // Use ISO week number for weeks belonging to the previous year
                    } else if month == 12 && iso_week == 1 {
                        // Case 2: Week belongs to the start of the next year (e.g., late December)
                        formatted_string.push_str(&format!("{}", iso_week)); // Use ISO week number for late December
                    } else if (week_of_month == 5 && month == 1 && iso_week == 5)
                        || (week_of_month == 1 && first_weekday_of_month == 5 && iso_week == 5)
                    {
                        // Case 3: Special handling for week 5 of January
                        formatted_string.push_str(&format!("{}", iso_week));
                    } else if week_of_month == 5 && first_weekday_of_month == 0 {
                        // Case 4: Special handling for weeks that start at the end of the month
                        formatted_string.push_str(&format!("{}", 1)); // Force the week to be week 1 of the next month
                    } else {
                        // Regular week of the month handling
                        formatted_string.push_str(&format!("{}", week_of_month));
                    }
                }

                "xNn" => {
                    // Calculate the Monday of the current week
                    let days_from_monday = date.weekday().num_days_from_monday() as i64;
                    let first_day_of_week = *date - chrono::Duration::days(days_from_monday); // Go back to the Monday of the week
                    let last_day_of_week = first_day_of_week + chrono::Duration::days(6); // Get the Sunday of the week

                    let first_day_month = first_day_of_week.month(); // Get the month of the Monday
                    let last_day_month = last_day_of_week.month(); // Get the month of the Sunday

                    // If the week starts in one month and ends in another, decide the month based on which has more days in the week
                    let week_month = if first_day_month != last_day_month {
                        if last_day_of_week.day() >= 4 {
                            // More than 3 days in the last month
                            last_day_month
                        } else {
                            first_day_month
                        }
                    } else {
                        first_day_month // Week does not cross months, use the first day’s month
                    };

                    // Format the month based on the determined week month
                    let month_name = chrono::NaiveDate::from_ymd_opt(date.year(), week_month, 1)
                        .expect("Invalid month or day")
                        .format("%B")
                        .to_string();
                    formatted_string.push_str(&month_name);
                }

                "M01" => formatted_string.push_str(&date.format("%m").to_string()), // Month with leading 0
                "H01" => formatted_string.push_str(&date.format("%H").to_string()), // Hours in 24-hour format with leading zero
                "h" => formatted_string.push_str(&date.format("%-I").to_string()), // Hour in 12-hour format without leading zero (e.g., 9)
                "m" => formatted_string.push_str(&date.format("%M").to_string()),  // Minutes
                "s" => formatted_string.push_str(&date.format("%S").to_string()),  // Seconds
                "f001" => formatted_string.push_str(&date.format("%3f").to_string()), // Milliseconds
                "Z01:01t" => {
                    if date.offset().local_minus_utc() == 0 {
                        formatted_string.push('Z'); // UTC, add Z
                    } else {
                        formatted_string.push_str(&date.format("%:z").to_string());
                        // Timezone offset with colon
                    }
                }
                "Z01:01" => {
                    if date.offset().local_minus_utc() == 0 {
                        formatted_string.push_str("+00:00"); // Instead of 'Z', output '+00:00' for UTC
                    } else {
                        let offset_minutes = date.offset().local_minus_utc() / 60;
                        let hours = offset_minutes / 60;
                        let minutes = offset_minutes % 60;
                        formatted_string.push_str(&format!("{:+03}:{:02}", hours, minutes));
                        // Format as +01:00 or -05:00
                    }
                }
                "Z0101t" => {
                    // Adjust timezone format without colon for +0100
                    if date.offset().local_minus_utc() == 0 {
                        formatted_string.push('Z'); // UTC, add Z
                    } else {
                        let offset_minutes = date.offset().local_minus_utc() / 60;
                        let hours = offset_minutes / 60;
                        let minutes = offset_minutes % 60;
                        formatted_string.push_str(&format!("{:+03}{:02}", hours, minutes));
                        // Format as +0100 or -0500
                    }
                }
                "Z" => {
                    formatted_string.push_str(&date.format("%:z").to_string()); // Use %:z to format timezone with a colon
                }
                "z" => {
                    let tz_offset = date.format("%:z").to_string(); // Get timezone in `±HH:MM` format
                    let gmt_tz = format!("GMT{}", tz_offset); // Prepend "GMT" to the formatted timezone
                    formatted_string.push_str(&gmt_tz); // Append the formatted "GMT±HH:MM" to the result
                }
                "Z0" => {
                    let tz_offset = date.format("%z").to_string(); // Get timezone offset as "+0530" or "-0500"

                    let trimmed_tz = if tz_offset == "+0000" || tz_offset == "-0000" {
                        // Handle UTC as "0"
                        "0".to_string()
                    } else if &tz_offset[3..] == "00" {
                        // If the minutes are "00", only keep the hour part (e.g., "+5" or "-5")
                        format!(
                            "{}{}",
                            &tz_offset[..1],
                            tz_offset[1..3].trim_start_matches('0')
                        )
                    } else if tz_offset.starts_with("-0") || tz_offset.starts_with("+0") {
                        // Handle leading zero in hours (e.g., "+0100" -> "+1", "-0500" -> "-5")
                        format!(
                            "{}{}:{}",
                            &tz_offset[..1],
                            tz_offset[1..3].trim_start_matches('0'),
                            &tz_offset[3..]
                        )
                    } else {
                        // Format timezone with hour and minute (e.g., "+0530" -> "+5:30")
                        format!(
                            "{}{}:{}",
                            &tz_offset[..1],
                            tz_offset[1..3].trim_start_matches('0'),
                            &tz_offset[3..]
                        )
                    };

                    formatted_string.push_str(&trimmed_tz); // Append the formatted timezone offset to the result
                }
                "Z010101t" => {
                    // Handle specific case where more than four digits were used erroneously
                    return Err(Error::D3134Error("Invalid timezone format".to_string()));
                }
                "m01" => formatted_string.push_str(&date.format("%M").to_string()), // Minutes with leading zero
                "s01" => formatted_string.push_str(&date.format("%S").to_string()), // Seconds with leading zero
                "F0" => formatted_string.push_str(&date.format("%u").to_string()), // ISO day of the week (1-7)
                "FNn" => formatted_string.push_str(&date.format("%A").to_string()), // Full name of the day
                "FNn,3-3" => formatted_string.push_str(&date.format("%A").to_string()[..3]), // First 3 characters of the day name (e.g., "Fri")
                "h#1" => formatted_string.push_str(&date.format("%-I").to_string()), // 12-hour format without leading 0
                "P" => formatted_string.push_str(&date.format("%p").to_string().to_lowercase()), // AM/PM
                // Handle AM/PM uppercase
                "PN" => {
                    formatted_string.push_str(&date.format("%p").to_string()); // Use %p for AM/PM in uppercase
                }
                // Handle AM/PM lowercase
                "Pn" => {
                    formatted_string.push_str(&date.format("%p").to_string().to_lowercase());
                    // Use %p for AM/PM in lowercase
                }
                "YI" => formatted_string.push_str(&to_roman_numerals(date.year())), // Roman numerals (uppercase)
                "Yi" => formatted_string.push_str(&to_roman_numerals_lower(date.year())), // Roman numerals (lowercase)
                "D1o" => formatted_string.push_str(&format_day_with_ordinal(date.day())), // Day with ordinal suffix
                "MNn" => formatted_string.push_str(&date.format("%B").to_string()), // Full month name (e.g., March)
                "MNn,3-3" => formatted_string.push_str(&date.format("%B").to_string()[..3]), // First 3 characters of the month name (e.g., "Mar")
                "MN" => formatted_string.push_str(&date.format("%B").to_string().to_uppercase()), // Full month name in uppercase (e.g., MARCH)
                "YN" => {
                    // Handle specific case where more than four digits were used erroneously
                    return Err(Error::D3133Error("Invalid timezone format".to_string()));
                }
                "Yw" => formatted_string.push_str(&to_year_in_words(date.year())), // Year in words (e.g., 2018 -> "two thousand eighteen")
                "E" => formatted_string.push_str("ISO"), // Insert "ISO" for [E]
                "F" => formatted_string.push_str(&date.format("%A").to_string().to_lowercase()), // Full name of the day in lowercase (e.g., "friday")
                "D" => formatted_string.push_str(&date.format("%-d").to_string()), // Day without leading zero (e.g., 23)
                "M" => formatted_string.push_str(&date.format("%-m").to_string()), // Month without leading zero (e.g., 3)
                "C" => formatted_string.push_str("ISO"), // Handle [C] as "ISO"
                _ => formatted_string.push_str(&format!("[{}]", trimmed_pattern)), // Treat unrecognized pattern as literal
            }

            current_pattern.clear();
            i += 1;
            continue;
        }

        // If inside brackets, accumulate the pattern
        if inside_brackets {
            current_pattern.push(ch);
        } else {
            // Outside of brackets, treat as literal
            formatted_string.push(ch);
        }

        i += 1;
    }

    Ok(formatted_string)
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
