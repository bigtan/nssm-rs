//! Command line splitting and quoting.
//!
//! `parse_command_line` and `quote_argument`/`join_arguments` are inverses:
//! arguments joined with `join_arguments` survive a round trip through
//! `parse_command_line`, including empty arguments, embedded spaces and
//! embedded quotes (escaped as `\"`, which matches what
//! `CommandLineToArgvW` understands for the simple cases we emit).

/// Split a command line into arguments.
///
/// Supports double-quoted sections, `\"` as an escaped quote and preserves
/// empty arguments written as `""`.
pub fn parse_command_line(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut in_quotes = false;
    let mut has_arg = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' if chars.peek() == Some(&'"') => {
                current_arg.push(chars.next().unwrap());
                has_arg = true;
            }
            '"' => {
                in_quotes = !in_quotes;
                has_arg = true;
            }
            ' ' | '\t' if !in_quotes => {
                if has_arg {
                    args.push(std::mem::take(&mut current_arg));
                    has_arg = false;
                }
            }
            _ => {
                current_arg.push(ch);
                has_arg = true;
            }
        }
    }

    if has_arg {
        args.push(current_arg);
    }

    args
}

/// Quote a single argument so it survives `parse_command_line` (and
/// `CommandLineToArgvW`) unchanged.
pub fn quote_argument(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        arg.to_string()
    } else {
        format!("\"{}\"", arg.replace('"', "\\\""))
    }
}

/// Join arguments into a single command line, quoting where necessary.
pub fn join_arguments<S: AsRef<str>>(arguments: &[S]) -> String {
    arguments
        .iter()
        .map(|arg| quote_argument(arg.as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_arguments() {
        assert_eq!(parse_command_line("a b c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_collapses_whitespace() {
        assert_eq!(parse_command_line("  a \t b  "), vec!["a", "b"]);
    }

    #[test]
    fn parse_quoted_argument_with_spaces() {
        assert_eq!(
            parse_command_line("\"a b\" c"),
            vec!["a b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn parse_preserves_empty_quoted_argument() {
        assert_eq!(parse_command_line("a \"\" b"), vec!["a", "", "b"]);
    }

    #[test]
    fn parse_escaped_quote() {
        assert_eq!(
            parse_command_line(r#"say \"hi\""#),
            vec!["say".to_string(), "\"hi\"".to_string()]
        );
    }

    #[test]
    fn parse_adjacent_quoted_segments() {
        assert_eq!(parse_command_line("\"a \"\"b\""), vec!["a b"]);
    }

    #[test]
    fn parse_hyphen_arguments() {
        assert_eq!(
            parse_command_line("--port 80 -v"),
            vec!["--port", "80", "-v"]
        );
    }

    #[test]
    fn quote_plain_argument_unchanged() {
        assert_eq!(quote_argument("abc"), "abc");
    }

    #[test]
    fn quote_argument_with_space() {
        assert_eq!(quote_argument("a b"), "\"a b\"");
    }

    #[test]
    fn quote_empty_argument() {
        assert_eq!(quote_argument(""), "\"\"");
    }

    #[test]
    fn quote_argument_with_quote() {
        assert_eq!(quote_argument("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn join_and_parse_round_trip() {
        let original = vec![
            "C:\\Program Files\\app.exe",
            "--name",
            "hello world",
            "",
            "x\"y",
        ];
        let joined = join_arguments(&original);
        assert_eq!(parse_command_line(&joined), original);
    }
}
