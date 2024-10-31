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

/// A wrapper type for a regex literal so that we can implement PartialEq
#[derive(Debug, Clone)]
pub struct RegexLiteral(regex::Regex);

impl RegexLiteral {
    pub(super) fn new(regex: regex::Regex) -> Self {
        Self(regex)
    }
}

impl Deref for RegexLiteral {
    type Target = regex::Regex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for RegexLiteral {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}
