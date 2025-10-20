use std::collections::HashMap;

use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::parse::Term;
use crate::register_managers::ArgRegisterManager;

pub fn compile_query(query_terms: Vec<Term>) -> Vec<WamInstr> {
    let mut declared_vars = HashMap::new();
    let mut arg_register_manager = ArgRegisterManager::new();
    
    query_terms
        .into_iter()
        .flat_map(|term| compile_query_term(term, &mut declared_vars, &mut arg_register_manager))
        .collect()
}

fn compile_query_term(
    term: Term,
    _declared_vars: &mut HashMap<String, WamReg>,
    arg_register_manager: &mut ArgRegisterManager,
) -> Vec<WamInstr> {
    match term {
        Term::Atom(name) => {
            vec![WamInstr::PutAtom {
                reg: arg_register_manager.get_next(),
                name,
            }]
        }
        _ => {
            todo!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compiler_bytecode::{WamInstr, WamReg},
        parse::query,
    };

    fn test_compile_query_helper(source: &str, expected: Vec<WamInstr>) {
        let parsed_query = query(source).unwrap().1;
        let instructions = compile_query(parsed_query);
        assert_eq!(instructions, expected);
    }

    #[test]
    fn query_atom() {
        test_compile_query_helper(
            "parent.",
            vec![WamInstr::PutAtom {
                name: "parent".to_string(),
                reg: WamReg::A(0),
            }],
        );
    }
}
