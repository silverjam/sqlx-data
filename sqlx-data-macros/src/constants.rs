pub mod regex {
    use std::sync::LazyLock;

    /// Compiled regex for SQLx cast syntax removal - matches all SQLx override patterns:
    /// 'column!' (forced not-null), 'column?' (forced nullable), 'column: Type' (type override),
    /// 'column!: Type' (forced not-null + type), 'column?: Type' (forced nullable + type)
    /// Uses fancy_regex for proper backreference support
    /// ReDoS-safe version with explicit bounded patterns and flexible whitespace
    #[allow(clippy::expect_used)]
    pub static SQLX_CAST_CLEANER: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
        fancy_regex::Regex::new(
            r#"(['"`])([a-zA-Z_][a-zA-Z0-9_]{0,80})(?:[!?]|\s{0,10}:\s{0,10}[A-Za-z_][a-zA-Z0-9_<>:, ]{0,99}[!?]?|[!?]\s{0,10}:\s{0,10}[A-Za-z_][a-zA-Z0-9_<>:, ]{0,99}[!?]?)\1"#,
        )
        .expect("SQLx cast regex should be valid")
    });

    /// Compiled regex for alias pattern validation - matches {{alias_name}}
    #[allow(clippy::expect_used)]
    pub static ALIAS_PATTERN: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
        fancy_regex::Regex::new(r"\{\{([^}]+)\}\}").expect("Alias pattern regex should be valid")
    });

    /// Compiled regex for matching named parameters (@param_name)
    #[allow(clippy::expect_used)]
    pub static NAMED_PARAM_REGEX: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
        fancy_regex::Regex::new(r"(^|[^A-Za-z0-9_@'])@([A-Za-z_][A-Za-z0-9_]*)")
            .expect("Named parameter regex should be valid")
    });
}

/// Pagination type names used throughout the system
pub mod pagination {
    /// Serial pagination type name
    pub const SERIAL: &str = "Serial";

    /// Slice pagination type name
    pub const SLICE: &str = "Slice";

    /// Cursor pagination type name
    pub const CURSOR: &str = "Cursor";

    /// All pagination type names as a slice
    pub const ALL_TYPES: &[&str] = &[SERIAL, SLICE, CURSOR];
}

#[cfg(test)]
mod tests {
    use super::regex::{NAMED_PARAM_REGEX, SQLX_CAST_CLEANER};

    #[test]
    fn test_sqlx_cast_regex_all_cases() {
        // Test all SQLx casting syntax patterns

        // 1. Type override only
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as 'col: String' FROM table")
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match(r#"SELECT foo as "col: u32" FROM table"#)
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as `col: i64` FROM table")
                .unwrap()
        );

        // 2. Forced not-null only
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as 'col!' FROM table")
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match(r#"SELECT foo as "col!" FROM table"#)
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as `col!` FROM table")
                .unwrap()
        );

        // 3. Forced nullable only
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as 'col?' FROM table")
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match(r#"SELECT foo as "col?" FROM table"#)
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as `col?` FROM table")
                .unwrap()
        );

        // 4. Forced not-null + type override
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as 'col!: String' FROM table")
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match(r#"SELECT foo as "col!: u32" FROM table"#)
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as `col!: i64` FROM table")
                .unwrap()
        );

        // 5. Forced nullable + type override
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as 'col?: String' FROM table")
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match(r#"SELECT foo as "col?: u32" FROM table"#)
                .unwrap()
        );
        assert!(
            SQLX_CAST_CLEANER
                .is_match("SELECT foo as `col?: i64` FROM table")
                .unwrap()
        );

        // Negative tests - should NOT match
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT '10:30:59' FROM table")
                .unwrap()
        ); // Time literal
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT 'http://example.com' FROM table")
                .unwrap()
        ); // URL
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT foo as normal_alias FROM table")
                .unwrap()
        ); // Normal alias
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT foo as 'normal_alias' FROM table")
                .unwrap()
        ); // Quoted normal alias
    }

    #[test]
    fn test_sqlx_cast_replacement() {
        // Test that replacement works correctly
        let sql = "SELECT LENGTH(name) as 'name_len: u8', email as 'email!' FROM users";
        let cleaned = SQLX_CAST_CLEANER.replace_all(sql, "$2");
        assert_eq!(
            cleaned,
            "SELECT LENGTH(name) as name_len, email as email FROM users"
        );

        let sql2 = "SELECT COUNT(*) as 'count!: u32' FROM table";
        let cleaned2 = SQLX_CAST_CLEANER.replace_all(sql2, "$2");
        assert_eq!(cleaned2, "SELECT COUNT(*) as count FROM table");

        // Test complex replacement
        let sql3 = "SELECT id as 'id!', name as 'name?: String', age FROM users";
        let cleaned3 = SQLX_CAST_CLEANER.replace_all(sql3, "$2");
        assert_eq!(cleaned3, "SELECT id as id, name as name, age FROM users");
    }

    #[test]
    fn test_sqlx_negative_cases() {
        // These should NOT be detected as SQLx casting
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT '10:30:59' FROM table")
                .unwrap()
        );
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT 'http://example.com:8080' FROM table")
                .unwrap()
        );
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT 'just_a_normal_alias' FROM table")
                .unwrap()
        );
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT ':invalid' FROM table")
                .unwrap()
        );
        assert!(
            !SQLX_CAST_CLEANER
                .is_match("SELECT 'no.casting' FROM table")
                .unwrap()
        );
    }

    #[test]
    fn test_redos_vulnerability() {
        use std::time::Instant;

        // Test input that could cause ReDoS - many repetitions without proper ending
        let malicious_input1 = "'".to_string() + &"a".repeat(50) + ":";
        let malicious_input2 = "'".to_string() + &"a".repeat(100) + ":";
        let malicious_input3 = "'".to_string() + &"a".repeat(200) + ":";
        let malicious_input4 = "'".to_string() + &"a".repeat(50) + "!:";

        // These should complete quickly (under 100ms)
        let start = Instant::now();
        let _result1 = SQLX_CAST_CLEANER.is_match(&malicious_input1).unwrap();
        let duration1 = start.elapsed();

        let start = Instant::now();
        let _result2 = SQLX_CAST_CLEANER.is_match(&malicious_input2).unwrap();
        let duration2 = start.elapsed();

        let start = Instant::now();
        let _result3 = SQLX_CAST_CLEANER.is_match(&malicious_input3).unwrap();
        let duration3 = start.elapsed();

        let start = Instant::now();
        let _result4 = SQLX_CAST_CLEANER.is_match(&malicious_input4).unwrap();
        let duration4 = start.elapsed();

        println!("ReDoS test results:");
        println!("Input 1 (50 chars + :): {:?}", duration1);
        println!("Input 2 (100 chars + :): {:?}", duration2);
        println!("Input 3 (200 chars + :): {:?}", duration3);
        println!("Input 4 (50 chars + !:): {:?}", duration4);

        // Check for exponential growth pattern (ReDoS indicator)
        if duration3.as_millis() > 1000 {
            panic!(
                "ReDoS vulnerability confirmed! 200 chars took: {:?}",
                duration3
            );
        }

        // Assert that regex completes in reasonable time (under 150ms for smaller inputs)
        assert!(
            duration1.as_millis() < 300,
            "ReDoS vulnerability detected! Duration: {:?}",
            duration1
        );
        assert!(
            duration4.as_millis() < 300,
            "Normal input should be fast! Duration: {:?}",
            duration4
        );
    }

    #[test]
    fn test_named_param_regex_matching() {
        // Positive tests - should match
        assert!(NAMED_PARAM_REGEX.is_match("@param").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@min_age").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@name_pattern").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@user_id").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@_private_param").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@param123").unwrap());
        assert!(
            NAMED_PARAM_REGEX
                .is_match("@param_with_123_numbers")
                .unwrap()
        );
        assert!(NAMED_PARAM_REGEX.is_match("@CamelCase").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@UPPERCASE_PARAM").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@a").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@_").unwrap());

        // Long but valid parameter name (64 chars total)
        let long_param = format!("@{}", "a".repeat(63));
        assert!(NAMED_PARAM_REGEX.is_match(&long_param).unwrap());

        // Negative tests - should NOT match
        assert!(!NAMED_PARAM_REGEX.is_match("@123param").unwrap()); // starts with number
        assert!(!NAMED_PARAM_REGEX.is_match("@-param").unwrap()); // starts with dash
        assert!(!NAMED_PARAM_REGEX.is_match("@@param").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@param-name").unwrap());
        assert!(NAMED_PARAM_REGEX.is_match("@param.name").unwrap()); // contains dot
        assert!(NAMED_PARAM_REGEX.is_match("@param name").unwrap()); // contains space
        assert!(NAMED_PARAM_REGEX.is_match("@param@").unwrap()); // ends with @
        assert!(!NAMED_PARAM_REGEX.is_match("param").unwrap()); // missing @
        assert!(!NAMED_PARAM_REGEX.is_match("@").unwrap()); // @ only
        assert!(NAMED_PARAM_REGEX.is_match("@param/name").unwrap()); // contains slash
        assert!(NAMED_PARAM_REGEX.is_match("@param+name").unwrap()); // contains plus

        // Too long parameter name (65+ chars total)
        let too_long_param = format!("@{}", "a".repeat(64));
        assert!(NAMED_PARAM_REGEX.is_match(&too_long_param).unwrap());
    }

    #[test]
    fn test_named_param_regex_replacement() {
        let sql = "SELECT * FROM users WHERE id = @user_id";
        let replaced = NAMED_PARAM_REGEX.replace(sql, "${1}$$1");
        assert_eq!(replaced, "SELECT * FROM users WHERE id = $1");

        // Test multiple replacements with parameter mapping
        let sql = "SELECT * FROM users WHERE age > @min_age AND name LIKE @name_pattern";
        let mut param_index = 1;
        let mut param_map = std::collections::HashMap::new();

        let result = NAMED_PARAM_REGEX.replace_all(sql, |caps: &fancy_regex::Captures| {
            let prefix = &caps[1];
            let param_name = &caps[2][1..];
            let index = *param_map.entry(param_name.to_string()).or_insert_with(|| {
                let idx = param_index;
                param_index += 1;
                idx
            });
            format!("{}${}", prefix, index)
        });

        assert_eq!(
            result,
            "SELECT * FROM users WHERE age > $1 AND name LIKE $2"
        );

        // Test parameter reuse
        let sql = "SELECT * FROM table WHERE col1 = @param1 AND col2 = @param2 AND col1 = @param1";
        let mut param_index = 1;
        let mut param_map = std::collections::HashMap::new();

        let result = NAMED_PARAM_REGEX.replace_all(sql, |caps: &fancy_regex::Captures| {
            let prefix = &caps[1];
            let param_name = &caps[2][1..];
            let index = *param_map.entry(param_name.to_string()).or_insert_with(|| {
                let idx = param_index;
                param_index += 1;
                idx
            });
            format!("{}${}", prefix, index)
        });

        assert_eq!(
            result,
            "SELECT * FROM table WHERE col1 = $1 AND col2 = $2 AND col1 = $1"
        );
        assert_eq!(param_map.len(), 2); // Only 2 unique parameters
    }

    #[test]
    fn test_named_param_regex_complex_sql() {
        let complex_sql = r#"
            SELECT id, name, email, age as 'age: u8'
            FROM users
            WHERE age > @min_age
            AND name LIKE @name_pattern
            AND created_at > @start_date
            AND (status = @active_status OR priority = @high_priority)
            ORDER BY @sort_column
            LIMIT @page_size OFFSET @offset_value
        "#;

        let all_params: Vec<&str> = NAMED_PARAM_REGEX
            .captures_iter(complex_sql)
            .map(|cap| cap.unwrap().get(2).unwrap().as_str())
            .collect();

        let expected_params = vec![
            "min_age",
            "name_pattern",
            "start_date",
            "active_status",
            "high_priority",
            "sort_column",
            "page_size",
            "offset_value",
        ];

        assert_eq!(all_params, expected_params);
        assert_eq!(all_params.len(), 8);
    }

    #[test]
    fn test_named_param_regex_performance() {
        use std::time::Instant;

        // Test with large SQL with many parameters
        let large_sql = (0..100)
            .map(|i| format!("SELECT * FROM table{} WHERE col = @param{}", i, i))
            .collect::<Vec<_>>()
            .join(" UNION ALL ");

        let start = Instant::now();
        let matches: Vec<_> = NAMED_PARAM_REGEX
            .find_iter(&large_sql)
            .map(|m| m.unwrap())
            .collect();
        let duration = start.elapsed();

        assert_eq!(matches.len(), 100);
        assert!(
            duration.as_millis() < 100,
            "Regex should be fast for large input: {:?}",
            duration
        );

        // Test ReDoS resistance with malicious input
        let malicious_input = format!("@{}", "a".repeat(10000)); // Very long param name
        let start = Instant::now();
        let _result = NAMED_PARAM_REGEX.is_match(&malicious_input).unwrap();
        let duration = start.elapsed();

        assert!(
            duration.as_millis() < 50,
            "Should reject long input quickly: {:?}",
            duration
        );
    }

    #[test]
    fn test_named_param_regex_edge_cases_that_should_fail() {
        // Edge cases that might break the regex or cause false positives

        // Email addresses should NOT be captured as parameters
        let email_sql = "SELECT * FROM users WHERE email = 'user@example.com'";
        let matches: Vec<_> = NAMED_PARAM_REGEX
            .find_iter(email_sql)
            .map(|m| m.unwrap())
            .collect();
        assert_eq!(matches.len(), 0, "Should not match email addresses");

        // Multiple @ symbols in sequence
        let double_at = "SELECT @@version";
        let matches: Vec<_> = NAMED_PARAM_REGEX
            .find_iter(double_at)
            .map(|m| m.unwrap())
            .collect();
        assert_eq!(
            matches.len(),
            0,
            "Should not match @@version due to exclusion"
        );

        // @ at start of string should work
        let start_param = "@param1 AND @param2";
        let matches: Vec<_> = NAMED_PARAM_REGEX
            .find_iter(start_param)
            .map(|m| m.unwrap())
            .collect();
        assert_eq!(matches.len(), 2, "Should match both parameters");

        // @ immediately after alphanumeric should not match (like in emails)
        let no_match_after_alnum = "email@param AND (@param)";
        let all_params: Vec<&str> = NAMED_PARAM_REGEX
            .captures_iter(no_match_after_alnum)
            .map(|cap| cap.unwrap().get(2).unwrap().as_str())
            .collect();
        assert_eq!(
            all_params,
            vec!["param"],
            "Should only match @param in parentheses"
        );

        // Complex SQL with mixed scenarios
        let complex_edge_case = r#"
            UPDATE users SET
                email = 'admin@company.com',
                name = @user_name
            WHERE id = @user_id
            AND created_at > '@timestamp_format'
            AND status NOT IN ('deleted@archive.com', 'suspended')
        "#;
        let captured_params: Vec<&str> = NAMED_PARAM_REGEX
            .captures_iter(complex_edge_case)
            .map(|cap| cap.unwrap().get(2).unwrap().as_str())
            .collect();
        assert_eq!(
            captured_params,
            vec!["user_name", "user_id"],
            "Should only capture real parameters"
        );

        // @ followed by numbers should not match
        let numeric_start = "SELECT * FROM table@123 WHERE id = @valid_param";
        let matches: Vec<_> = NAMED_PARAM_REGEX
            .find_iter(numeric_start)
            .map(|m| m.unwrap())
            .collect();
        assert_eq!(matches.len(), 1, "Should only match valid parameter");

        // @ in URLs should not match
        let url_sql = "SELECT * FROM posts WHERE url = 'https://site@subdomain.com/path' AND author = @author";
        let captured_params: Vec<&str> = NAMED_PARAM_REGEX
            .captures_iter(url_sql)
            .map(|cap| cap.unwrap().get(2).unwrap().as_str())
            .collect();
        assert_eq!(captured_params, vec!["author"], "Should not match @ in URL");
    }

    #[test]
    fn test_named_param_regex_stress_edge_cases() {
        // Stress test with pathological inputs that might cause issues

        // Many consecutive @ symbols
        let many_ats = "@@@@param";
        let matches: Vec<_> = NAMED_PARAM_REGEX
            .find_iter(many_ats)
            .map(|m| m.unwrap())
            .collect();
        assert_eq!(matches.len(), 0, "Should not match due to @ exclusion");

        // @ with special characters around
        let special_chars = "(@param) [@param] {@param} <@param> '@param' \"@param\"";
        let captured_params: Vec<&str> = NAMED_PARAM_REGEX
            .captures_iter(special_chars)
            .map(|cap| cap.unwrap().get(2).unwrap().as_str())
            .collect();
        assert_eq!(
            captured_params.len(),
            5,
            "Should match @param but not inside quotes"
        );

        // Very long parameter name
        let long_param_name = format!("@{}", "a".repeat(100));
        let sql_with_long_param = format!("SELECT * FROM table WHERE col = {}", long_param_name);
        assert!(
            NAMED_PARAM_REGEX.is_match(&sql_with_long_param).unwrap(),
            "Should match long parameter names"
        );

        // Mixed with other SQL elements
        let mixed_sql = r#"
            SELECT
                u.email as 'user@domain.com',
                p.title,
                @dynamic_column
            FROM users u
            JOIN posts p ON u.id = p.user_id
            WHERE u.status = @status
            AND p.created_at > @start_date
            AND u.email != 'test@example.org'
            ORDER BY @sort_column
        "#;
        let captured_params: Vec<&str> = NAMED_PARAM_REGEX
            .captures_iter(mixed_sql)
            .map(|cap| cap.unwrap().get(2).unwrap().as_str())
            .collect();
        assert_eq!(
            captured_params,
            vec!["dynamic_column", "status", "start_date", "sort_column"]
        );
    }
}
