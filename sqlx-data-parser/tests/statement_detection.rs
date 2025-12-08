use sqlparser::ast::Statement;
use sqlx_data_parser::parse_sql;
use std::sync::Arc;

/// Helper function for tests to parse SQL and unwrap everything
fn parse_sql_for_tests(sql: &str) -> Arc<Statement> {
    parse_sql(sql).unwrap().unwrap()
}

fn test_statement_detection() {
    let test_sqls = vec![
        ("SELECT * FROM users", "Query"),
        ("SELECT id, name FROM users WHERE id = $1", "Query"),
        ("INSERT INTO users (name) VALUES ($1)", "Insert"),
        ("UPDATE users SET name = $2 WHERE id = $1", "Update"),
        ("DELETE FROM users WHERE id = $1", "Delete"),
        ("CREATE TABLE users (id INTEGER, name TEXT)", "CreateTable"),
    ];

    println!("\n=== Testing SQL Statement Detection ===");
    for (sql, expected) in test_sqls {
        println!("\nSQL: {}", sql);

        match parse_sql(sql) {
            Ok(Some(statement)) => {
                let stmt = statement.as_ref();
                let stmt_type = get_statement_type(stmt);
                println!("  Statement {} (expected: {})", stmt_type, expected);

                // Show more detail for each type
                match stmt {
                    Statement::Query(_) => {
                        println!("    Query details: has SELECT");
                    }
                    Statement::Insert(insert) => {
                        println!("    Insert into table: {}", insert.table);
                    }
                    Statement::Update(update) => {
                        println!("    Update table: {}", update.table);
                    }
                    Statement::Delete(delete) => {
                        println!("    Delete from table: {:?}", delete.from);
                    }
                    _ => {
                        println!("    Other statement type");
                    }
                }
            }
            Ok(None) => println!("  Parse failed: SQL could not be parsed"),
            Err(e) => println!("  Parse error: {}", e),
        }
    }
}

fn get_statement_type(stmt: &Statement) -> &'static str {
    match stmt {
        Statement::Query(_) => "Query (SELECT)",
        Statement::Insert(_) => "Insert",
        Statement::Update(_) => "Update",
        Statement::Delete(_) => "Delete",
        Statement::CreateTable(_) => "CreateTable",
        Statement::Drop { .. } => "Drop",
        _ => "Other",
    }
}

#[test]
fn test_sql_statement_types() {
    test_statement_detection();
}

#[test]
fn test_individual_statements() {
    // Test SELECT
    let select_sql = "SELECT id, name FROM users WHERE active = true";
    let statement = parse_sql_for_tests(select_sql);
    assert!(matches!(statement.as_ref(), Statement::Query(_)));

    // Test INSERT
    let insert_sql = "INSERT INTO users (name, email) VALUES ($1, $2)";
    let statement = parse_sql_for_tests(insert_sql);
    assert!(matches!(statement.as_ref(), Statement::Insert(_)));

    // Test UPDATE
    let update_sql = "UPDATE users SET name = $2 WHERE id = $1";
    let statement = parse_sql_for_tests(update_sql);
    assert!(matches!(statement.as_ref(), Statement::Update(_)));

    // Test DELETE
    let delete_sql = "DELETE FROM users WHERE id = $1";
    let statement = parse_sql_for_tests(delete_sql);
    assert!(matches!(statement.as_ref(), Statement::Delete(_)));
}
