use prolog::{
    parse::{database, query},
    term_rewrite::Interpreter,
};

fn main() {
    // Sample database and query
    let db_str = "parent(john, doe). parent(doe, jane).";
    let query_str = "parent(john, X).";

    // Parse database and query
    let db_clauses = database(db_str).expect("Failed to parse database");
    let (_, query_terms) = query(query_str).expect("Failed to parse query");

    println!("Database clauses: {:#?}", db_clauses);
    println!("Query terms: {:?}", query_terms);

    // Create interpreter and execute
    let mut interpreter = Interpreter::new(db_clauses);
    let result = interpreter.execute(query_terms);

    println!("Execution result: {:?}", result);
}