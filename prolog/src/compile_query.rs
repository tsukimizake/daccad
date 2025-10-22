use std::collections::HashMap;
use std::iter::once;

use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::parse::Term;
use crate::register_managers::ArgRegisterManager;

pub fn compile_query(query_terms: Vec<Term>) -> Vec<WamInstr> {
    let mut declared_vars = HashMap::new();
    let mut arg_reg_manager = ArgRegisterManager::new();

    query_terms
        .into_iter()
        .flat_map(|term| compile_query_term(term, &mut declared_vars, &mut arg_reg_manager))
        .collect()
}

fn compile_query_term(
    term: Term,
    declared_vars: &mut HashMap<String, WamReg>,
    arg_reg_manager: &mut ArgRegisterManager,
) -> Vec<WamInstr> {
    match term {
        Term::TopAtom(name) => {
            vec![WamInstr::CallTemp {
                predicate: name,
                arity: 0,
            }]
        }
        Term::InnerAtom(name) => {
            vec![WamInstr::PutAtom {
                reg: arg_reg_manager.get_next(),
                name,
            }]
        }
        Term::Var(name) => {
            vec![WamInstr::PutVar {
                name: name,
                reg: arg_reg_manager.get_next(),
            }]
        }

        Term::TopStruct { functor, args } => {
            let last = WamInstr::CallTemp {
                predicate: functor,
                arity: args.len(),
            };
            let rest = args
                .into_iter()
                .flat_map(|arg| compile_query_term(arg, declared_vars, arg_reg_manager));
            rest.chain(once(last)).collect()
        }

        Term::InnerStruct { functor, args } => {
            let head = WamInstr::PutStruct {
                functor: functor,
                arity: args.len(),
                reg: arg_reg_manager.get_next(),
            };
            let rest = args
                .into_iter()
                .flat_map(|arg| compile_query_term(arg, declared_vars, arg_reg_manager));

            once(head).chain(rest).collect()
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
        compiler_bytecode::WamInstr,
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
            vec![WamInstr::CallTemp {
                predicate: "parent".to_string(),
                arity: 0,
            }],
        );
    }

    #[test]
    fn query_top_atom() {
        test_compile_query_helper(
            "hello.",
            vec![WamInstr::CallTemp {
                predicate: "hello".to_string(),
                arity: 0,
            }],
        );
    }
}
