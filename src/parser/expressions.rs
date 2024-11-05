use regress::Regex;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

pub fn check_balanced_brackets(expr: &str) -> Result<(), String> {
    let mut bracket_count = 0;
    let mut i = 0;
    let chars: Vec<char> = expr.chars().collect();

    while i < chars.len() {
        let ch = chars[i];

        if ch == '[' {
            // Check if it's an escaped opening bracket ([[ -> literal [)
            if i + 1 < chars.len() && chars[i + 1] == '[' {
                i += 1; // Skip the next [
            } else {
                bracket_count += 1; // Regular opening bracket
            }
        } else if ch == ']' {
            // Check if it's an escaped closing bracket (]] -> literal ])
            if i + 1 < chars.len() && chars[i + 1] == ']' {
                i += 1; // Skip the next ]
            } else {
                if bracket_count == 0 {
                    return Err(format!("Unbalanced closing bracket found in: {}", expr));
                }
                bracket_count -= 1; // Regular closing bracket
            }
        }

        i += 1;
    }

    if bracket_count != 0 {
        return Err(format!(
            "No matching closing bracket ']' in expression: {}",
            expr
        ));
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct RegexLiteral {
    regex: Regex,
    pattern: String, // Store the original pattern string for comparisons
}

impl RegexLiteral {
    /// Create a new `RegexLiteral` with optional case-insensitive and multiline flags.
    pub fn new(
        pattern: &str,
        case_insensitive: bool,
        multi_line: bool,
    ) -> Result<Self, regress::Error> {
        // Add flags to the pattern string as needed
        let mut flags = String::new();
        if case_insensitive {
            flags.push('i');
        }
        if multi_line {
            flags.push('m');
        }
        let regex = Regex::with_flags(pattern, flags.as_str())?;
        Ok(Self {
            regex,
            pattern: pattern.to_string(),
        })
    }

    /// Check if the regex pattern matches a given text.
    pub fn is_match(&self, text: &str) -> bool {
        self.regex.find(text).is_some()
    }

    /// Retrieve the original pattern string for display purposes.
    pub fn as_pattern(&self) -> &str {
        &self.pattern
    }

    /// Get a reference to the inner `regress::Regex`.
    pub fn get_regex(&self) -> &Regex {
        &self.regex
    }
}

impl Deref for RegexLiteral {
    type Target = Regex;

    fn deref(&self) -> &Self::Target {
        &self.regex
    }
}

impl PartialEq for RegexLiteral {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Eq for RegexLiteral {}

impl Hash for RegexLiteral {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pattern.hash(state);
    }
}
