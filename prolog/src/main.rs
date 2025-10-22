use prolog::{
    compile_db::compile_db,
    compile_query::compile_query,
    interpreter::execute_instructions,
    parse::{database, query},
};

fn main() {
    // Sample database and query to suppress unused warnings
    let db_str = "hello.";
    let query_str = "hello.";

    // Parse database and query
    let db_clauses = database(db_str).expect("Failed to parse database");
    let (_, query_terms) = query(query_str).expect("Failed to parse query");

    // Compile database and query (this suppresses unused warnings)
    let db_instructions = compile_db(db_clauses);
    let query_instructions = compile_query(query_terms);

    // Execute through interpreter (uses the compiled instructions)
    let result = execute_instructions(db_instructions);

    println!("Database executed successfully: {:?}", result);
    println!(
        "Query compiled successfully: {} instructions",
        query_instructions.len()
    );
}

