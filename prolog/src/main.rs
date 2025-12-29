use prolog::{
    compile_db::compile_db,
    compile_link::compile_link,
    compile_query::compile_query,
    interpreter::execute_instructions,
    parse::{database, query},
};

fn main() {
    // Sample database and query to test compile_link
    let db_str = "parent(john, doe). parent(doe, jane).";
    let query_str = "parent(john, X).";

    // Parse database and query
    let db_clauses = database(db_str).expect("Failed to parse database");
    let (_, query_terms) = query(query_str).expect("Failed to parse query");

    // Compile database and query
    let db_instructions = compile_db(db_clauses);
    let query_instructions = compile_query(query_terms.clone());

    println!("DB instructions: {:#?}", db_instructions);
    println!("Query instructions: {:#?}", query_instructions);

    // Link the instructions using compile_link
    let linked_instructions = compile_link(query_instructions, db_instructions);

    println!("Linked instructions: {:#?}", linked_instructions);

    // Execute through interpreter (uses the linked instructions)
    let result = execute_instructions(linked_instructions, query_terms);

    println!("Execution result: {:?}", result);
}
