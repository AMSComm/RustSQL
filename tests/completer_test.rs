use rustsql::repl::completer::SqlCompleter;

#[test]
fn test_sql_keyword_completion() {
    let completer = SqlCompleter::new();

    // "sel" -> SELECT
    let (start, matches) = completer.complete_input("sel", 3);
    assert_eq!(start, 0);
    assert!(matches.iter().any(|m| m.replacement == "SELECT "));

    // "ord" -> ORDER BY
    let (start, matches) = completer.complete_input("ord", 3);
    assert_eq!(start, 0);
    assert!(matches.iter().any(|m| m.replacement == "ORDER BY "));

    // "GRO" -> GROUP BY
    let (start, matches) = completer.complete_input("GRO", 3);
    assert_eq!(start, 0);
    assert!(matches.iter().any(|m| m.replacement == "GROUP BY "));
}

#[test]
fn test_meta_command_completion() {
    let completer = SqlCompleter::new();

    // "\d" -> \d, \dt, \dv, \dn, etc.
    let (start, matches) = completer.complete_input("\\d", 2);
    assert_eq!(start, 0);
    assert!(matches.iter().any(|m| m.display == "\\d"));
    assert!(matches.iter().any(|m| m.display == "\\dt"));
    assert!(matches.iter().any(|m| m.display == "\\dv"));
    assert!(matches.iter().any(|m| m.display == "\\dn"));

    // "\p" -> \pset
    let (start, matches) = completer.complete_input("\\p", 2);
    assert_eq!(start, 0);
    assert!(matches.iter().any(|m| m.display == "\\pset"));
}

#[test]
fn test_meta_command_args_completion() {
    let completer = SqlCompleter::new();
    completer.update_cache(
        vec!["users".to_string(), "orders".to_string()],
        vec!["public".to_string(), "analytics".to_string()],
        vec!["v_summary".to_string()],
        vec!["dev".to_string(), "prod".to_string()],
        vec!["id".to_string(), "email".to_string()],
    );

    // "\c " -> dev, prod
    let (start, matches) = completer.complete_input("\\c ", 3);
    assert_eq!(start, 3);
    assert!(matches.iter().any(|m| m.display == "dev"));
    assert!(matches.iter().any(|m| m.display == "prod"));

    // "\c d" -> dev
    let (start, matches) = completer.complete_input("\\c d", 4);
    assert_eq!(start, 3);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].replacement, "dev ");

    // "\dt " -> users, orders
    let (start, matches) = completer.complete_input("\\dt ", 4);
    assert_eq!(start, 4);
    assert!(matches.iter().any(|m| m.display == "users"));
    assert!(matches.iter().any(|m| m.display == "orders"));

    // "\dn " -> public, analytics
    let (start, matches) = completer.complete_input("\\dn ", 4);
    assert_eq!(start, 4);
    assert!(matches.iter().any(|m| m.display == "public"));
    assert!(matches.iter().any(|m| m.display == "analytics"));

    // "\pset " -> border, format, tuples_only, etc.
    let (start, matches) = completer.complete_input("\\pset ", 6);
    assert_eq!(start, 6);
    assert!(matches.iter().any(|m| m.display == "border"));
    assert!(matches.iter().any(|m| m.display == "format"));
    assert!(matches.iter().any(|m| m.display == "tuples_only"));

    // "\pset border " -> 0, 1, 2
    let (start, matches) = completer.complete_input("\\pset border ", 13);
    assert_eq!(start, 13);
    assert!(matches.iter().any(|m| m.display == "1"));

    // "\pset format " -> csv, tsv, json, etc.
    let (start, matches) = completer.complete_input("\\pset format ", 13);
    assert_eq!(start, 13);
    assert!(matches.iter().any(|m| m.display == "csv"));
    assert!(matches.iter().any(|m| m.display == "json"));
}

#[test]
fn test_contextual_sql_completion() {
    let completer = SqlCompleter::new();
    completer.update_cache(
        vec![
            "users".to_string(),
            "public.users".to_string(),
            "orders".to_string(),
        ],
        vec!["public".to_string()],
        vec![],
        vec![],
        vec!["user_id".to_string(), "created_at".to_string()],
    );

    // "SELECT * FROM " -> users, orders, public
    let (start, matches) = completer.complete_input("SELECT * FROM ", 14);
    assert_eq!(start, 14);
    assert!(matches.iter().any(|m| m.display == "users"));
    assert!(matches.iter().any(|m| m.display == "orders"));
    assert!(matches.iter().any(|m| m.display == "public"));

    // "SELECT * FROM u" -> users
    let (start, matches) = completer.complete_input("SELECT * FROM u", 15);
    assert_eq!(start, 14);
    assert!(matches.iter().any(|m| m.replacement == "users "));

    // "SELECT * FROM public." -> public.users
    let (start, matches) = completer.complete_input("SELECT * FROM public.", 21);
    assert_eq!(start, 14);
    assert!(matches.iter().any(|m| m.display == "public.users"));

    // "SELECT " -> *, DISTINCT, user_id, created_at
    let (start, matches) = completer.complete_input("SELECT ", 7);
    assert_eq!(start, 7);
    assert!(matches.iter().any(|m| m.display == "*"));
    assert!(matches.iter().any(|m| m.display == "DISTINCT"));
    assert!(matches.iter().any(|m| m.display == "user_id"));

    // "SELECT * FROM users ORDER " -> BY
    let (start, matches) = completer.complete_input("SELECT * FROM users ORDER ", 26);
    assert_eq!(start, 26);
    assert_eq!(matches[0].replacement, "BY ");
}

#[test]
fn test_function_and_procedure_completion() {
    let completer = SqlCompleter::new();
    completer.update_cache(
        vec!["users".to_string(), "public.users".to_string()],
        vec!["public".to_string(), "analytics".to_string()],
        vec![],
        vec![],
        vec![],
    );
    completer.set_functions(vec![
        "calc_total".to_string(),
        "public.calc_total".to_string(),
        "sp_refresh".to_string(),
        "public.sp_refresh".to_string(),
    ]);

    // SQL prefix completion
    let (start, matches) = completer.complete_input("SELECT calc_", 12);
    assert_eq!(start, 7);
    assert!(matches.iter().any(|m| m.display == "calc_total"));

    // CALL procedure completion
    let (start, matches) = completer.complete_input("CALL ", 5);
    assert_eq!(start, 5);
    assert!(matches.iter().any(|m| m.display == "sp_refresh"));

    // Dot-qualified function completion
    let (start, matches) = completer.complete_input("SELECT public.calc", 18);
    assert_eq!(start, 7);
    assert!(matches.iter().any(|m| m.display == "public.calc_total"));

    // Meta-command \df / \sf completion
    let (start, matches) = completer.complete_input("\\sf sp_", 7);
    assert_eq!(start, 4);
    assert!(matches.iter().any(|m| m.display == "sp_refresh"));

    // Lazy schema merge
    completer.merge_schema_objects(
        "analytics",
        vec!["analytics.events".to_string()],
        vec![],
        vec!["analytics.agg_events".to_string()],
    );
    let (_, matches) = completer.complete_input("SELECT * FROM analytics.", 24);
    assert!(matches.iter().any(|m| m.display == "analytics.events"));
    assert!(matches.iter().any(|m| m.display == "analytics.agg_events"));
}

#[test]
fn test_table_and_alias_column_completion_in_where() {
    let completer = SqlCompleter::new();
    completer.set_table_columns(
        "public.users",
        vec![
            "id".to_string(),
            "email".to_string(),
            "status".to_string(),
        ],
    );
    completer.set_table_columns(
        "orders",
        vec!["order_id".to_string(), "total_amount".to_string()],
    );

    // WHERE after space -> columns of referenced table
    let (_, matches) = completer.complete_input("SELECT * FROM users WHERE ", 26);
    assert!(matches.iter().any(|m| m.display == "email"));
    assert!(matches.iter().any(|m| m.display == "status"));
    assert!(!matches.iter().any(|m| m.display == "total_amount"));

    // WHERE with prefix -> matching column
    let (start, matches) = completer.complete_input("SELECT * FROM public.users WHERE em", 35);
    assert_eq!(start, 33);
    assert!(matches.iter().any(|m| m.display == "email"));

    // Alias dot completion in WHERE / ON
    let (start, matches) =
        completer.complete_input("SELECT * FROM public.users u JOIN orders o ON u.", 48);
    assert_eq!(start, 46);
    assert!(matches.iter().any(|m| m.display == "u.email"));
    assert!(!matches.iter().any(|m| m.display == "u.total_amount"));

    // ORDER BY completion
    let (_, matches) = completer.complete_input("SELECT * FROM orders ORDER BY ", 30);
    assert!(matches.iter().any(|m| m.display == "total_amount"));
}
