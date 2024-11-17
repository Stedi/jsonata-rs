// Acknowledgement:
//
// This is based on the JSON dumping code from [Maciej Hirsz's](https://github.com/maciejhirsz)
// excellent [json crate](https://github.com/maciejhirsz/json-rust), and modified to work on
// our custom internal value.
//
// The original code is licensed in the same way as this crate.

use std::io::Write;

use super::Value;
use crate::Result;

const QU: u8 = b'"';
const BS: u8 = b'\\';
const BB: u8 = b'b';
const TT: u8 = b't';
const NN: u8 = b'n';
const FF: u8 = b'f';
const RR: u8 = b'r';
const UU: u8 = b'u';
const __: u8 = 0;

// Look up table for characters that need escaping in a product string
static ESCAPED: [u8; 256] = [
    // 0   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
    UU, UU, UU, UU, UU, UU, UU, UU, BB, TT, NN, UU, FF, RR, UU, UU, // 0
    UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, // 1
    __, __, QU, __, __, __, __, __, __, __, __, __, __, __, __, __, // 2
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 3
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 4
    __, __, __, __, __, __, __, __, __, __, __, __, BS, __, __, __, // 5
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 6
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 7
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 8
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 9
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // A
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // B
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // C
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // D
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // E
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // F
];

pub trait Formatter {
    fn write_min(&self, output: &mut Vec<u8>, slice: &[u8], min: u8);
    fn new_line(&self, output: &mut Vec<u8>);
    fn indent(&mut self);
    fn dedent(&mut self);
}

pub struct DumpFormatter;

impl Formatter for DumpFormatter {
    #[inline(always)]
    fn write_min(&self, output: &mut Vec<u8>, _: &[u8], min: u8) {
        output.push(min);
    }

    #[inline(always)]
    fn new_line(&self, _output: &mut Vec<u8>) {}

    #[inline(always)]
    fn indent(&mut self) {}

    #[inline(always)]
    fn dedent(&mut self) {}
}

pub struct PrettyFormatter {
    dent: u16,
    spaces: u16,
}

impl Default for PrettyFormatter {
    fn default() -> Self {
        Self { dent: 0, spaces: 2 }
    }
}

impl Formatter for PrettyFormatter {
    #[inline(always)]
    fn write_min(&self, output: &mut Vec<u8>, slice: &[u8], _: u8) {
        output.extend_from_slice(slice);
    }

    fn new_line(&self, output: &mut Vec<u8>) {
        output.push(b'\n');
        for _ in 0..(self.dent * self.spaces) {
            output.push(b' ');
        }
    }

    fn indent(&mut self) {
        self.dent += 1;
    }

    fn dedent(&mut self) {
        self.dent -= 1;
    }
}

pub struct Serializer<T: Formatter> {
    output: Vec<u8>,
    formatter: T,
    fail_on_invalid_numbers: bool,
}

impl<T: Formatter> Serializer<T> {
    pub fn new(formatter: T, fail_on_invalid_numbers: bool) -> Self {
        Serializer {
            output: Vec::with_capacity(1024),
            formatter,
            fail_on_invalid_numbers,
        }
    }

    pub fn serialize<'a>(mut self, value: &'a Value<'a>) -> Result<String> {
        self.write_json(value)?;

        // SAFETY: Original strings were unicode, numbers are all ASCII,
        // therefore this is safe.
        Ok(unsafe { String::from_utf8_unchecked(self.output) })
    }

    #[inline(always)]
    fn write(&mut self, slice: &[u8]) {
        self.output.extend_from_slice(slice);
    }

    #[inline(always)]
    fn write_char(&mut self, ch: u8) {
        self.output.push(ch);
    }

    #[inline(never)]
    fn write_string_complex(&mut self, string: &str, mut start: usize) {
        self.write(&string.as_bytes()[..start]);

        for (index, ch) in string.bytes().enumerate().skip(start) {
            let escape = ESCAPED[ch as usize];
            if escape > 0 {
                self.write(&string.as_bytes()[start..index]);
                self.write(&[b'\\', escape]);
                start = index + 1;
            }
            if escape == b'u' {
                write!(self.output, "{:04x}", ch).unwrap();
            }
        }
        self.write(&string.as_bytes()[start..]);

        self.write_char(b'"');
    }

    #[inline(always)]
    fn write_string(&mut self, string: &str) {
        self.write_char(b'"');

        for (index, ch) in string.bytes().enumerate() {
            if ESCAPED[ch as usize] > 0 {
                self.write_string_complex(string, index);
                return;
            }
        }

        self.write(string.as_bytes());
        self.write_char(b'"');
    }

    #[inline(always)]
    fn write_number(&mut self, number: f64) {
        const MAX_SIGNIFICANT_DIGITS: usize = 15;

        if number.is_finite() {
            let mut buffer = dtoa::Buffer::new();
            let formatted = buffer.format_finite(number).as_bytes();

            // JSONata uses JSON.stringify with Number.toPrecision(15) to format numbers.
            //
            // dtoa gets us close to the behaviour of JSON.stringify, in particular for
            // switching to scientific notation (which Rust format! doesn't do), but dtoa
            // doesn't support specifying a number of significant digits.
            //
            // This craziness limits the number of significant digits and trims off trailing
            // zeroes in the fraction by doing string manipulation.
            //
            // It's not pretty, and I'm sure there's a better way to do this.
            let mut split_iter = formatted.split(|b| *b == b'.');
            let whole = split_iter.next();
            let fraction = split_iter.next();
            if let Some(whole) = whole {
                self.write(whole);
                if whole.len() < MAX_SIGNIFICANT_DIGITS {
                    if let Some(fraction) = fraction {
                        let fraction_length =
                            usize::min(MAX_SIGNIFICANT_DIGITS - whole.len(), fraction.len());
                        if fraction_length > 0 {
                            let fraction = unsafe {
                                std::str::from_utf8_unchecked(&fraction[0..fraction_length])
                                    .trim_end_matches('0')
                            };
                            if !fraction.is_empty() {
                                self.write_char(b'.');
                                self.write(fraction.as_bytes());
                            }
                        }
                    }
                }
            } else {
                self.write(formatted);
            }
        } else {
            self.write(b"null");
        }
    }

    #[inline(always)]
    fn write_object<'a>(&mut self, object: &'a Value<'a>) -> Result<()> {
        self.write_char(b'{');
        let mut iter = object.entries();

        if let Some((key, value)) = iter.next() {
            self.formatter.indent();
            self.formatter.new_line(&mut self.output);
            self.write_string(key);
            self.formatter.write_min(&mut self.output, b": ", b':');
            self.write_json(value)?;
        } else {
            self.write_char(b'}');
            return Ok(());
        }

        for (key, value) in iter {
            self.write_char(b',');
            self.formatter.new_line(&mut self.output);
            self.write_string(key);
            self.formatter.write_min(&mut self.output, b": ", b':');
            self.write_json(value)?;
        }

        self.formatter.dedent();
        self.formatter.new_line(&mut self.output);
        self.write_char(b'}');

        Ok(())
    }

    #[inline(always)]
    fn write_array<'a>(&mut self, array: &'a Value<'a>) -> Result<()> {
        self.write_char(b'[');
        let mut iter = array.members();

        if let Some(item) = iter.next() {
            self.formatter.indent();
            self.formatter.new_line(&mut self.output);
            self.write_json(item)?;
        } else {
            self.write_char(b']');
            return Ok(());
        }

        for item in iter {
            self.write_char(b',');
            self.formatter.new_line(&mut self.output);
            self.write_json(item)?;
        }

        self.formatter.dedent();
        self.formatter.new_line(&mut self.output);
        self.write_char(b']');

        Ok(())
    }

    fn write_json<'a>(&mut self, value: &'a Value<'a>) -> Result<()> {
        match value {
            Value::Undefined => {}
            Value::Null => self.write(b"null"),
            Value::String(ref string) => self.write_string(string),
            Value::Number(n) => {
                if self.fail_on_invalid_numbers {
                    value.is_valid_number()?;
                }
                self.write_number(*n);
            }
            Value::Bool(true) => self.write(b"true"),
            Value::Bool(false) => self.write(b"false"),
            Value::Array(..) | Value::Range(..) => self.write_array(value)?,
            Value::Object(..) => self.write_object(value)?,
            Value::Regex(ref regex) => {
                let pattern = format!("\"{}\"", regex.as_pattern());
                self.write(pattern.as_bytes());
            }
            Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. } => {
                self.write(b"\"\"")
            }
        };

        Ok(())
    }
}
