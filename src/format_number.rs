//! `$formatNumber`: formats a number with an XPath F&O 3.1 picture string (section 4.7,
//! `fn:format-number`), following the reference JSONata implementation, including the cases
//! where it departs from the letter of the spec, so results match jsonata-js.
//!
//! Everything works on `char`s, since the decimal digit family, the separators and the
//! prefix and suffix can all be outside ASCII (e.g. `{"zero-digit": "⑟"}`).

use crate::{Error, Result};

/// The decimal format properties a picture is read with; `$formatNumber`'s third argument
/// overrides any of them.
#[derive(Debug, Clone)]
pub struct DecimalFormat {
    pub decimal_separator: String,
    pub grouping_separator: String,
    pub exponent_separator: String,
    pub minus_sign: String,
    pub percent: String,
    pub per_mille: String,
    pub zero_digit: String,
    pub digit: String,
    pub pattern_separator: String,
}

impl Default for DecimalFormat {
    fn default() -> Self {
        Self {
            decimal_separator: ".".to_string(),
            grouping_separator: ",".to_string(),
            exponent_separator: "e".to_string(),
            minus_sign: "-".to_string(),
            percent: "%".to_string(),
            per_mille: "\u{2030}".to_string(),
            zero_digit: "0".to_string(),
            digit: "#".to_string(),
            pattern_separator: ";".to_string(),
        }
    }
}

impl DecimalFormat {
    /// Overrides one property by its JSONata name. `infinity` and `NaN` are accepted and have
    /// no effect, as in jsonata-js (a JSON number is never either); other names are ignored.
    pub fn set(&mut self, name: &str, value: &str) {
        let value = value.to_string();
        match name {
            "decimal-separator" => self.decimal_separator = value,
            "grouping-separator" => self.grouping_separator = value,
            "exponent-separator" => self.exponent_separator = value,
            "minus-sign" => self.minus_sign = value,
            "percent" => self.percent = value,
            "per-mille" => self.per_mille = value,
            "zero-digit" => self.zero_digit = value,
            "digit" => self.digit = value,
            "pattern-separator" => self.pattern_separator = value,
            _ => {}
        }
    }
}

type Chars = Vec<char>;

fn chars(s: &str) -> Chars {
    s.chars().collect()
}

/// `haystack.indexOf(needle, from)`.
fn find(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from.min(haystack.len()));
    }
    if needle.len() > haystack.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

/// `haystack.lastIndexOf(needle)`.
fn rfind(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(haystack.len());
    }
    if needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len())
        .rev()
        .find(|&i| haystack[i..i + needle.len()] == *needle)
}

fn contains(haystack: &[char], needle: &[char]) -> bool {
    find(haystack, needle, 0).is_some()
}

/// `s.substring(start, end)`: both ends clamped to the length, and swapped when reversed.
fn substring(s: &[char], start: usize, end: usize) -> Chars {
    let (start, end) = (start.min(s.len()), end.min(s.len()));
    let (start, end) = if start > end {
        (end, start)
    } else {
        (start, end)
    };
    s[start..end].to_vec()
}

/// A position as `Array.prototype.slice` reads it: negative counts from the end.
fn slice_index(index: isize, len: usize) -> usize {
    if index < 0 {
        (len as isize + index).max(0) as usize
    } else {
        (index as usize).min(len)
    }
}

/// `s.slice(0, at) + insert + s.slice(at)`.
fn insert_at(s: &mut Chars, at: isize, insert: &[char]) {
    let at = slice_index(at, s.len());
    s.splice(at..at, insert.iter().copied());
}

/// One character compared to a property, as jsonata-js compares a one-character string to it.
fn is(ch: char, property: &[char]) -> bool {
    property.len() == 1 && property[0] == ch
}

struct Symbols {
    decimal_separator: Chars,
    grouping_separator: Chars,
    exponent_separator: Chars,
    minus_sign: Chars,
    percent: Chars,
    per_mille: Chars,
    digit: Chars,
    pattern_separator: Chars,
    /// The ten digits from the zero digit up.
    digits: [char; 10],
}

impl Symbols {
    fn new(format: &DecimalFormat) -> Self {
        let zero = format.zero_digit.chars().next().unwrap_or('0') as u32;
        let digits = std::array::from_fn(|i| char::from_u32(zero + i as u32).unwrap_or('0'));
        Self {
            decimal_separator: chars(&format.decimal_separator),
            grouping_separator: chars(&format.grouping_separator),
            exponent_separator: chars(&format.exponent_separator),
            minus_sign: chars(&format.minus_sign),
            percent: chars(&format.percent),
            per_mille: chars(&format.per_mille),
            digit: chars(&format.digit),
            pattern_separator: chars(&format.pattern_separator),
            digits,
        }
    }

    fn zero(&self) -> char {
        self.digits[0]
    }

    fn is_decimal_digit(&self, ch: char) -> bool {
        self.digits.contains(&ch)
    }

    fn is_digit_or_optional(&self, ch: char) -> bool {
        self.is_decimal_digit(ch) || is(ch, &self.digit)
    }

    fn is_active(&self, ch: char) -> bool {
        self.is_decimal_digit(ch)
            || is(ch, &self.decimal_separator)
            || is(ch, &self.exponent_separator)
            || is(ch, &self.grouping_separator)
            || is(ch, &self.digit)
            || is(ch, &self.pattern_separator)
    }

    fn count_decimal_digits(&self, s: &[char]) -> usize {
        s.iter().filter(|&&ch| self.is_decimal_digit(ch)).count()
    }
}

/// A sub-picture split into its parts (F&O 4.7.3).
#[derive(Debug, Clone)]
struct Parts {
    prefix: Chars,
    suffix: Chars,
    active: Chars,
    mantissa: Chars,
    exponent: Option<Chars>,
    integer: Chars,
    fractional: Chars,
    subpicture: Chars,
}

fn split_parts(sym: &Symbols, subpicture: Chars) -> Parts {
    let is_boundary = |ch: char| sym.is_active(ch) && !is(ch, &sym.exponent_separator);
    let prefix = match subpicture.iter().position(|&ch| is_boundary(ch)) {
        Some(i) => subpicture[..i].to_vec(),
        None => Vec::new(),
    };
    let suffix = match subpicture.iter().rposition(|&ch| is_boundary(ch)) {
        Some(i) => subpicture[i + 1..].to_vec(),
        None => Vec::new(),
    };
    let active = substring(&subpicture, prefix.len(), subpicture.len() - suffix.len());
    let exponent_position = find(&subpicture, &sym.exponent_separator, prefix.len())
        .filter(|&position| position <= subpicture.len() - suffix.len());
    // As in jsonata-js, the exponent position found in the sub-picture slices the active part.
    let (mantissa, exponent) = match exponent_position {
        None => (active.clone(), None),
        Some(position) => (
            substring(&active, 0, position),
            Some(substring(&active, position + 1, active.len())),
        ),
    };
    let (integer, fractional) = match find(&mantissa, &sym.decimal_separator, 0) {
        // As in jsonata-js, a mantissa without a decimal separator takes the suffix as its
        // fractional part.
        None => (mantissa.clone(), suffix.clone()),
        Some(position) => (
            substring(&mantissa, 0, position),
            substring(&mantissa, position + 1, mantissa.len()),
        ),
    };
    Parts {
        prefix,
        suffix,
        active,
        mantissa,
        exponent,
        integer,
        fractional,
        subpicture,
    }
}

/// Checks a sub-picture (F&O 4.7.3). Like jsonata-js, every rule is checked and the last one
/// broken is reported.
fn validate(sym: &Symbols, parts: &Parts, char_index: usize) -> Result<()> {
    let picture = &parts.subpicture;
    let mut error: Option<fn(usize) -> Error> = None;
    let decimal_position = find(picture, &sym.decimal_separator, 0);
    if decimal_position != rfind(picture, &sym.decimal_separator) {
        error = Some(Error::D3081MultipleDecimalSeparators);
    }
    if find(picture, &sym.percent, 0) != rfind(picture, &sym.percent) {
        error = Some(Error::D3082MultiplePercent);
    }
    if find(picture, &sym.per_mille, 0) != rfind(picture, &sym.per_mille) {
        error = Some(Error::D3083MultiplePerMille);
    }
    if contains(picture, &sym.percent) && contains(picture, &sym.per_mille) {
        error = Some(Error::D3084PercentAndPerMille);
    }
    if !parts
        .mantissa
        .iter()
        .any(|&ch| sym.is_digit_or_optional(ch))
    {
        error = Some(Error::D3085MantissaWithoutDigit);
    }
    if parts.active.iter().any(|&ch| !sym.is_active(ch)) {
        error = Some(Error::D3086PassiveBetweenActive);
    }
    let char_at =
        |i: isize| -> Option<char> { (i >= 0).then(|| picture.get(i as usize).copied()).flatten() };
    let is_grouping = |ch: Option<char>| ch.is_some_and(|ch| is(ch, &sym.grouping_separator));
    match decimal_position {
        Some(position) => {
            let position = position as isize;
            if is_grouping(char_at(position - 1)) || is_grouping(char_at(position + 1)) {
                error = Some(Error::D3087GroupingNextToDecimal);
            }
        }
        None => {
            if is_grouping(parts.integer.last().copied()) {
                error = Some(Error::D3088GroupingAtEndOfInteger);
            }
        }
    }
    let doubled = [
        sym.grouping_separator.clone(),
        sym.grouping_separator.clone(),
    ]
    .concat();
    if contains(picture, &doubled) {
        error = Some(Error::D3089AdjacentGroupingSeparators);
    }
    if let Some(position) = find(&parts.integer, &sym.digit, 0) {
        if sym.count_decimal_digits(&parts.integer[..position]) > 0 {
            error = Some(Error::D3090DigitBeforeOptionalDigit);
        }
    }
    if let Some(position) = rfind(&parts.fractional, &sym.digit) {
        if sym.count_decimal_digits(&parts.fractional[position.min(parts.fractional.len())..]) > 0 {
            error = Some(Error::D3091OptionalDigitBeforeDigit);
        }
    }
    if let Some(exponent) = &parts.exponent {
        if !exponent.is_empty()
            && (contains(picture, &sym.percent) || contains(picture, &sym.per_mille))
        {
            error = Some(Error::D3092ExponentWithPercent);
        }
        if exponent.is_empty() || exponent.iter().any(|&ch| !sym.is_decimal_digit(ch)) {
            error = Some(Error::D3093InvalidExponent);
        }
    }
    match error {
        Some(error) => Err(error(char_index)),
        None => Ok(()),
    }
}

/// What formatting needs from a sub-picture (F&O 4.7.4).
#[derive(Debug, Clone)]
struct Variables {
    integer_grouping_positions: Vec<usize>,
    regular_grouping: usize,
    minimum_integer_size: usize,
    scaling_factor: usize,
    fractional_grouping_positions: Vec<usize>,
    minimum_fractional_size: usize,
    maximum_fractional_size: usize,
    minimum_exponent_size: usize,
    prefix: Chars,
    suffix: Chars,
    picture: Chars,
}

fn grouping_positions(sym: &Symbols, parts: &Parts, part: &[char], to_left: bool) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut position = find(part, &sym.grouping_separator, 0);
    while let Some(at) = position {
        let side = if to_left {
            &part[..at.min(part.len())]
        } else {
            &part[at.min(part.len())..]
        };
        positions.push(
            side.iter()
                .filter(|&&ch| sym.is_digit_or_optional(ch))
                .count(),
        );
        // As in jsonata-js, the next separator is looked for in the integer part.
        position = find(&parts.integer, &sym.grouping_separator, at + 1);
    }
    positions
}

/// The interval of regularly spaced grouping positions, or 0 when they are not regular.
fn regular_grouping(positions: &[usize]) -> usize {
    fn gcd(a: usize, b: usize) -> usize {
        if b == 0 {
            a
        } else {
            gcd(b, a % b)
        }
    }
    let Some(&first) = positions.first() else {
        return 0;
    };
    let factor = positions.iter().skip(1).fold(first, |a, &b| gcd(a, b));
    let regular = (1..=positions.len()).all(|index| positions.contains(&(index * factor)));
    if regular {
        factor
    } else {
        0
    }
}

fn analyse(sym: &Symbols, parts: &Parts) -> Variables {
    let integer_grouping_positions = grouping_positions(sym, parts, &parts.integer, false);
    let regular_grouping = regular_grouping(&integer_grouping_positions);
    let fractional_grouping_positions = grouping_positions(sym, parts, &parts.fractional, true);

    let mut minimum_integer_size = sym.count_decimal_digits(&parts.integer);
    let scaling_factor = minimum_integer_size;
    let mut minimum_fractional_size = sym.count_decimal_digits(&parts.fractional);
    let mut maximum_fractional_size = parts
        .fractional
        .iter()
        .filter(|&&ch| sym.is_digit_or_optional(ch))
        .count();
    let exponent_present = parts.exponent.is_some();
    if minimum_integer_size == 0 && maximum_fractional_size == 0 {
        if exponent_present {
            minimum_fractional_size = 1;
            maximum_fractional_size = 1;
        } else {
            minimum_integer_size = 1;
        }
    }
    if exponent_present && minimum_integer_size == 0 && contains(&parts.integer, &sym.digit) {
        minimum_integer_size = 1;
    }
    if minimum_integer_size == 0 && minimum_fractional_size == 0 {
        minimum_fractional_size = 1;
    }
    let minimum_exponent_size = parts
        .exponent
        .as_ref()
        .map_or(0, |exponent| sym.count_decimal_digits(exponent));

    Variables {
        integer_grouping_positions,
        regular_grouping,
        minimum_integer_size,
        scaling_factor,
        fractional_grouping_positions,
        minimum_fractional_size,
        maximum_fractional_size,
        minimum_exponent_size,
        prefix: parts.prefix.clone(),
        suffix: parts.suffix.clone(),
        picture: parts.subpicture.clone(),
    }
}

/// Rounds half to even at `precision` decimal places, shifting through the decimal
/// exponent as `$round` does, so the shift adds no binary floating point error.
fn round(value: f64, precision: usize) -> f64 {
    let shift = |value: f64, power: isize| -> f64 {
        format!("{value:e}")
            .split_once('e')
            .and_then(|(mantissa, exponent)| {
                let exponent: isize = exponent.parse().ok()?;
                format!("{mantissa}e{}", exponent + power).parse().ok()
            })
            .unwrap_or(value)
    };
    let shifted = shift(value, precision as isize).round_ties_even();
    let result = shift(shifted, -(precision as isize));
    if result == 0.0 {
        0.0
    } else {
        result
    }
}

/// `Math.abs(value).toFixed(places)`, written in the decimal digit family.
fn fixed(sym: &Symbols, value: f64, places: usize) -> Chars {
    format!("{:.*}", places, value.abs())
        .chars()
        .map(|ch| match ch.to_digit(10) {
            Some(d) => sym.digits[d as usize],
            None => ch,
        })
        .collect()
}

/// Formats `value` with `picture`, as `$formatNumber(value, picture, options)` does.
///
/// `char_index` is where the call is in the expression, for errors.
pub fn format_number(
    value: f64,
    picture: &str,
    format: &DecimalFormat,
    char_index: usize,
) -> Result<String> {
    let sym = Symbols::new(format);
    let picture = chars(picture);

    let sub_pictures: Vec<Chars> = if sym.pattern_separator.is_empty() {
        picture.iter().map(|&ch| vec![ch]).collect()
    } else {
        let mut pieces = Vec::new();
        let mut start = 0;
        while let Some(at) = find(&picture, &sym.pattern_separator, start) {
            pieces.push(picture[start..at].to_vec());
            start = at + sym.pattern_separator.len();
        }
        pieces.push(picture[start..].to_vec());
        pieces
    };
    if sub_pictures.len() > 2 {
        return Err(Error::D3080TooManySubPictures(char_index));
    }

    let parts: Vec<Parts> = sub_pictures
        .into_iter()
        .map(|sub| split_parts(&sym, sub))
        .collect();
    for part in &parts {
        validate(&sym, part, char_index)?;
    }
    let mut variables: Vec<Variables> = parts.iter().map(|part| analyse(&sym, part)).collect();
    if variables.len() == 1 {
        let mut negative = variables[0].clone();
        negative.prefix = [sym.minus_sign.clone(), negative.prefix].concat();
        variables.push(negative);
    }

    let pic = if value >= 0.0 {
        &variables[0]
    } else {
        &variables[1]
    };

    let adjusted = if contains(&pic.picture, &sym.percent) {
        value * 100.0
    } else if contains(&pic.picture, &sym.per_mille) {
        value * 1000.0
    } else {
        value
    };

    let (mantissa, exponent) = if pic.minimum_exponent_size == 0 {
        (adjusted, None)
    } else {
        let max_mantissa = 10f64.powi(pic.scaling_factor as i32);
        let min_mantissa = 10f64.powi(pic.scaling_factor as i32 - 1);
        let mut mantissa = adjusted;
        let mut exponent: i32 = 0;
        // Zero has exponent zero (F&O 4.7.5, rule 5); comparing magnitudes keeps a negative
        // mantissa from looping forever.
        if mantissa != 0.0 && mantissa.is_finite() {
            while mantissa.abs() < min_mantissa {
                mantissa *= 10.0;
                exponent -= 1;
            }
            while mantissa.abs() > max_mantissa {
                mantissa /= 10.0;
                exponent += 1;
            }
        }
        (mantissa, Some(exponent))
    };

    let rounded = round(mantissa, pic.maximum_fractional_size);
    let zero = sym.zero();
    let decimal = &sym.decimal_separator;

    let mut s = fixed(&sym, rounded, pic.maximum_fractional_size);
    match s.iter().position(|&ch| ch == '.') {
        None => s.extend(decimal.iter().copied()),
        Some(at) => {
            s.splice(at..=at, decimal.iter().copied());
        }
    }
    let leading = s.iter().take_while(|&&ch| ch == zero).count();
    s.drain(..leading);
    let trailing = s.iter().rev().take_while(|&&ch| ch == zero).count();
    s.truncate(s.len() - trailing);

    // Rules 8 and 9: pad to the minimum integer and fractional sizes.
    let decimal_position = find(&s, decimal, 0).unwrap_or(s.len()) as isize;
    let pad_left = pic.minimum_integer_size as isize - decimal_position;
    let pad_right =
        pic.minimum_fractional_size as isize - (s.len() as isize - decimal_position - 1);
    if pad_left > 0 {
        s.splice(0..0, std::iter::repeat_n(zero, pad_left as usize));
    }
    if pad_right > 0 {
        s.extend(std::iter::repeat_n(zero, pad_right as usize));
    }

    // Rule 10: integer part grouping.
    let mut decimal_position = find(&s, decimal, 0).unwrap_or(s.len()) as isize;
    if pic.regular_grouping > 0 {
        let interval = pic.regular_grouping as isize;
        let groups = (decimal_position - 1).div_euclid(interval);
        for group in 1..=groups {
            insert_at(
                &mut s,
                decimal_position - group * interval,
                &sym.grouping_separator,
            );
        }
    } else {
        for &position in &pic.integer_grouping_positions {
            insert_at(
                &mut s,
                decimal_position - position as isize,
                &sym.grouping_separator,
            );
            decimal_position += 1;
        }
    }

    // Rule 11: fractional part grouping.
    let decimal_position = find(&s, decimal, 0).unwrap_or(s.len()) as isize;
    for &position in &pic.fractional_grouping_positions {
        insert_at(
            &mut s,
            position as isize + decimal_position + 1,
            &sym.grouping_separator,
        );
    }

    // Rule 12: drop a decimal separator with nothing after it, or one the picture has not.
    let decimal_position = find(&s, decimal, 0);
    if !contains(&pic.picture, decimal) || decimal_position == Some(s.len().saturating_sub(1)) {
        s.pop();
    }

    // Rule 13: the exponent.
    if let Some(exponent) = exponent {
        let mut digits = fixed(&sym, exponent as f64, 0);
        let pad = pic.minimum_exponent_size.saturating_sub(digits.len());
        digits.splice(0..0, std::iter::repeat_n(zero, pad));
        s.extend(sym.exponent_separator.iter().copied());
        if exponent < 0 {
            s.extend(sym.minus_sign.iter().copied());
        }
        s.extend(digits);
    }

    // Rule 14: prefix and suffix.
    let mut result: String = pic.prefix.iter().collect();
    result.extend(s);
    result.extend(pic.suffix.iter());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(value: f64, picture: &str) -> String {
        format_number(value, picture, &DecimalFormat::default(), 0).unwrap()
    }

    fn code(value: f64, picture: &str) -> String {
        format_number(value, picture, &DecimalFormat::default(), 0)
            .unwrap_err()
            .code()
            .to_string()
    }

    #[test]
    fn groups_pads_and_rounds() {
        assert_eq!(fmt(12345.6, "#,###.00"), "12,345.60");
        assert_eq!(fmt(1234567.891, "#,##0.00"), "1,234,567.89");
        assert_eq!(fmt(0.5, "0"), "0");
        assert_eq!(fmt(1.5, "0"), "2");
        assert_eq!(fmt(2.675, "0.00"), "2.68");
        assert_eq!(fmt(0.0, "#,##0.00"), "0.00");
        assert_eq!(fmt(-1234.5, "#,##0.0"), "-1,234.5");
        assert_eq!(fmt(7.0, "000"), "007");
        assert_eq!(fmt(5.0, "#"), "5");
    }

    #[test]
    fn negative_sub_picture_and_affixes() {
        assert_eq!(fmt(-5.0, "0.00;(0.00)"), "(5.00)");
        assert_eq!(fmt(5.0, "0.00;(0.00)"), "5.00");
        assert_eq!(fmt(12.5, "$#,##0.00 USD"), "$12.50 USD");
    }

    #[test]
    fn exponents_of_zero_and_negative_numbers_terminate() {
        assert_eq!(fmt(0.0, "0.0e0"), "0.0e0");
        assert_eq!(fmt(-1234.5678, "00.000e0"), "-12.346e2");
        assert_eq!(fmt(-0.00012, "0.0e0"), "-1.2e-4");
    }

    #[test]
    fn options_change_every_symbol() {
        let mut format = DecimalFormat::default();
        format.set("decimal-separator", ",");
        format.set("grouping-separator", ".");
        format.set("NaN", "ignored");
        assert_eq!(
            format_number(1234567.891, "#.##0,00", &format, 0).unwrap(),
            "1.234.567,89"
        );
    }

    #[test]
    fn the_last_rule_broken_is_reported() {
        assert_eq!(code(1.0, "#;#;#"), "D3080");
        assert_eq!(code(1.0, "#.0.0"), "D3081");
        // Two decimal separators and two percent signs: the later rule wins, as in jsonata-js.
        assert_eq!(code(1.0, "#.0.0%%"), "D3082");
        assert_eq!(code(1.0, "0,"), "D3088");
    }
}
