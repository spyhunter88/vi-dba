use serde_json::Value;

/// Metadata about a parsed SQL query, used for context switching and result processing.
pub struct QueryInfo {
    pub q_trimmed: String,
    pub is_select: bool,
    pub detected_table_name: Option<String>,
}

/// Parses a SQL query string to determine if it's a "SELECT-like" operation 
/// and attempts to extract the table name for primary key lookups.
pub fn parse_query(query: &str, explicit_table: Option<String>) -> QueryInfo {
    let q_trimmed = query.trim().to_uppercase();
    let is_select = q_trimmed.starts_with("SELECT") || 
                   q_trimmed.starts_with("SHOW") || 
                   q_trimmed.starts_with("DESCRIBE") || 
                   q_trimmed.starts_with("EXPLAIN") ||
                   q_trimmed.starts_with("WITH") ||
                   q_trimmed.starts_with("PRAGMA");

    let detected_table_name = if let Some(table) = explicit_table {
        Some(table)
    } else if is_select {
        // Basic table name extraction for SELECT * FROM [table]
        if q_trimmed.starts_with("SELECT * FROM ") || q_trimmed.starts_with("SELECT * FROM `") || q_trimmed.starts_with("SELECT * FROM [") || q_trimmed.starts_with("SELECT * FROM \"") {
            let parts: Vec<&str> = query.split_whitespace().collect();
            if parts.len() >= 4 {
                Some(parts[3].trim_matches(|c| c == '`' || c == '"' || c == '[' || c == ']' || c == ';').to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    QueryInfo {
        q_trimmed,
        is_select,
        detected_table_name,
    }
}

/// Resolves the effective database/schema context based on priority 
/// (explicitly provided catalog/schema vs connection default).
pub fn get_effective_context(catalog: Option<String>, schema: Option<String>, default: Option<String>) -> Option<String> {
    catalog.filter(|s| !s.is_empty())
        .or(schema.filter(|s| !s.is_empty()))
        .or(default.filter(|s| !s.is_empty()))
}

/// Splits a SQL script into individual statements on top-level semicolons.
///
/// The splitter is dialect-agnostic but aware of the common constructs that make
/// a naive `split(';')` unsafe: single/double-quoted strings, backtick-quoted
/// identifiers, `--` line comments, `/* */` block comments, PostgreSQL
/// dollar-quoted strings (`$$ ... $$` / `$tag$ ... $tag$`), and backslash escapes
/// inside strings (MySQL). Semicolons inside any of these are preserved.
///
/// Leading/trailing whitespace is trimmed from each statement and empty
/// statements (e.g. from trailing semicolons or comment-only fragments) are
/// dropped.
pub fn split_sql_statements(sql: &str) -> Vec<String> {
    #[derive(PartialEq)]
    enum State {
        Normal,
        Single,
        Double,
        Backtick,
        LineComment,
        BlockComment,
    }

    let chars: Vec<char> = sql.chars().collect();
    let n = chars.len();
    let mut statements: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut state = State::Normal;
    // When inside a dollar-quoted string this holds the opening tag (e.g. "$$"
    // or "$body$"); the string ends when the same tag is seen again.
    let mut dollar_tag: Option<String> = None;
    let mut i = 0;

    while i < n {
        let c = chars[i];
        let next = chars.get(i + 1).copied();

        // Dollar-quoted string content takes precedence over every other state.
        if let Some(tag) = &dollar_tag {
            if c == '$' {
                if let Some(matched) = match_dollar_tag(&chars, i) {
                    if &matched == tag {
                        current.push_str(&matched);
                        i += matched.chars().count();
                        dollar_tag = None;
                        continue;
                    }
                }
            }
            current.push(c);
            i += 1;
            continue;
        }

        match state {
            State::Normal => match c {
                '\'' => {
                    state = State::Single;
                    current.push(c);
                    i += 1;
                }
                '"' => {
                    state = State::Double;
                    current.push(c);
                    i += 1;
                }
                '`' => {
                    state = State::Backtick;
                    current.push(c);
                    i += 1;
                }
                '-' if next == Some('-') => {
                    state = State::LineComment;
                    current.push_str("--");
                    i += 2;
                }
                '/' if next == Some('*') => {
                    state = State::BlockComment;
                    current.push_str("/*");
                    i += 2;
                }
                '$' => {
                    if let Some(matched) = match_dollar_tag(&chars, i) {
                        dollar_tag = Some(matched.clone());
                        current.push_str(&matched);
                        i += matched.chars().count();
                    } else {
                        current.push(c);
                        i += 1;
                    }
                }
                ';' => {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        statements.push(trimmed);
                    }
                    current.clear();
                    i += 1;
                }
                _ => {
                    current.push(c);
                    i += 1;
                }
            },
            State::Single => {
                if c == '\\' && next.is_some() {
                    current.push(c);
                    current.push(next.unwrap());
                    i += 2;
                } else if c == '\'' {
                    if next == Some('\'') {
                        // Escaped quote ('') — stays inside the string.
                        current.push('\'');
                        current.push('\'');
                        i += 2;
                    } else {
                        state = State::Normal;
                        current.push(c);
                        i += 1;
                    }
                } else {
                    current.push(c);
                    i += 1;
                }
            }
            State::Double => {
                if c == '\\' && next.is_some() {
                    current.push(c);
                    current.push(next.unwrap());
                    i += 2;
                } else if c == '"' {
                    if next == Some('"') {
                        current.push('"');
                        current.push('"');
                        i += 2;
                    } else {
                        state = State::Normal;
                        current.push(c);
                        i += 1;
                    }
                } else {
                    current.push(c);
                    i += 1;
                }
            }
            State::Backtick => {
                if c == '`' {
                    state = State::Normal;
                }
                current.push(c);
                i += 1;
            }
            State::LineComment => {
                if c == '\n' {
                    state = State::Normal;
                }
                current.push(c);
                i += 1;
            }
            State::BlockComment => {
                if c == '*' && next == Some('/') {
                    current.push_str("*/");
                    i += 2;
                    state = State::Normal;
                } else {
                    current.push(c);
                    i += 1;
                }
            }
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }
    statements
}

/// Attempts to read a PostgreSQL dollar-quote tag starting at `start` (which must
/// point at a `$`). Returns the full tag including both `$` delimiters (e.g. `$$`
/// or `$body$`) if valid, or `None` if this `$` does not begin a tag (e.g. a `$1`
/// parameter placeholder).
fn match_dollar_tag(chars: &[char], start: usize) -> Option<String> {
    if chars.get(start) != Some(&'$') {
        return None;
    }
    // The `$$` (empty tag) case.
    if chars.get(start + 1) == Some(&'$') {
        return Some("$$".to_string());
    }
    let mut tag = String::from("$");
    let mut j = start + 1;
    let mut first = true;
    while j < chars.len() {
        let ch = chars[j];
        if ch == '$' {
            tag.push('$');
            return Some(tag);
        }
        let ok = if first {
            ch.is_alphabetic() || ch == '_'
        } else {
            ch.is_alphanumeric() || ch == '_'
        };
        if !ok {
            return None;
        }
        tag.push(ch);
        first = false;
        j += 1;
    }
    None
}

/// Converts a serde_json::Value into an optional String for SQL binding.
pub fn val_to_opt_string(val: &Value) -> Option<String> {
    match val {
        Value::Null => None,
        Value::String(s) => if s.is_empty() { None } else { Some(s.clone()) },
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => Some(val.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::split_sql_statements;

    #[test]
    fn splits_simple_statements() {
        let got = split_sql_statements("SELECT 1; SELECT 2;");
        assert_eq!(got, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn drops_trailing_and_empty_statements() {
        let got = split_sql_statements("SELECT 1;;  ; ");
        assert_eq!(got, vec!["SELECT 1"]);
    }

    #[test]
    fn keeps_single_statement_without_semicolon() {
        let got = split_sql_statements("SELECT 1");
        assert_eq!(got, vec!["SELECT 1"]);
    }

    #[test]
    fn ignores_semicolon_in_single_quotes() {
        let got = split_sql_statements("INSERT INTO t VALUES ('a;b'); SELECT 1");
        assert_eq!(got, vec!["INSERT INTO t VALUES ('a;b')", "SELECT 1"]);
    }

    #[test]
    fn ignores_escaped_quotes() {
        // '' escape and \' backslash escape both stay inside the string.
        let got = split_sql_statements("SELECT 'it''s;fine'; SELECT 'a\\';b'");
        assert_eq!(got, vec!["SELECT 'it''s;fine'", "SELECT 'a\\';b'"]);
    }

    #[test]
    fn ignores_semicolon_in_comments() {
        let got =
            split_sql_statements("SELECT 1; -- comment; still\nSELECT 2; /* block; here */ SELECT 3");
        assert_eq!(
            got,
            vec![
                "SELECT 1",
                "-- comment; still\nSELECT 2",
                "/* block; here */ SELECT 3"
            ]
        );
    }

    #[test]
    fn handles_dollar_quoted_body() {
        let script = "CREATE FUNCTION f() RETURNS int AS $$ BEGIN; RETURN 1; END; $$ LANGUAGE plpgsql; SELECT f()";
        let got = split_sql_statements(script);
        assert_eq!(got.len(), 2);
        assert!(got[0].contains("BEGIN; RETURN 1; END;"));
        assert_eq!(got[1], "SELECT f()");
    }

    #[test]
    fn handles_tagged_dollar_quote() {
        let script = "SELECT $body$ a;b $body$; SELECT 2";
        let got = split_sql_statements(script);
        assert_eq!(got, vec!["SELECT $body$ a;b $body$", "SELECT 2"]);
    }

    #[test]
    fn dollar_param_is_not_a_tag() {
        // $1 is a placeholder, not a dollar-quote opener.
        let got = split_sql_statements("SELECT $1; SELECT $2");
        assert_eq!(got, vec!["SELECT $1", "SELECT $2"]);
    }

    #[test]
    fn transaction_block_splits_into_statements() {
        let script = "BEGIN;\nCREATE TEMPORARY TABLE tmp (id int);\nINSERT INTO tmp VALUES (1);\nCOMMIT;";
        let got = split_sql_statements(script);
        assert_eq!(
            got,
            vec![
                "BEGIN",
                "CREATE TEMPORARY TABLE tmp (id int)",
                "INSERT INTO tmp VALUES (1)",
                "COMMIT"
            ]
        );
    }
}
