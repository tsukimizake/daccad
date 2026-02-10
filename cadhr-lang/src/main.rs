use cadhr_lang::{
    parse::{database, query},
    term_rewrite::execute,
};

fn main() {
    // Sample database and query
    let db_str = "parent(john, doe). parent(doe, jane).";
    let query_str = "parent(john, X).";

    // Parse database and query
    let mut db_clauses = database(db_str).expect("Failed to parse database");
    let (_, query_terms) = query(query_str).expect("Failed to parse query");

    info!("Database clauses: {:#?}", db_clauses);
    info!("Query terms: {:?}", query_terms);

    // Execute query
    let result = execute(&mut db_clauses, query_terms);

    info!("Execution result: {:?}", result);
}
