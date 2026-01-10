use std::collections::HashMap;
use std::iter::once;

use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::parse::{Term, TermId};
use crate::register_managers::{RegisterManager, alloc_registers};

pub fn compile_query(query_terms: Vec<Term>) -> Vec<WamInstr> {
    query_terms
        .into_iter()
        .flat_map(|term| compile_query_term(term))
        .collect()
}

fn compile_query_term(term: Term) -> Vec<WamInstr> {
    let mut declared_vars = HashMap::new();
    let mut reg_manager = RegisterManager::new();
    alloc_registers(&term, &mut declared_vars, &mut reg_manager);
    compile_defs(&term, &declared_vars)
}

fn compile_defs(term: &Term, reg_map: &HashMap<TermId, WamReg>) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args, .. } => {
            let functor_children = args
                .iter()
                .filter(|arg| matches!(arg, Term::Struct { .. }))
                .flat_map(|arg| compile_defs(arg, reg_map));

            let key = term.id();

            functor_children
                .chain(once(WamInstr::PutStruct {
                    functor: functor.clone(),
                    arity: args.len(),
                    reg: reg_map[&key],
                }))
                .chain(args.iter().map(|arg| {
                    let reg = reg_map[&arg.id()];
                    WamInstr::SetVal {
                        name: arg.get_name().to_string(),
                        reg,
                    }
                }))
                .collect()
        }
        Term::Var { name, .. } => {
            let key = term.id();
            vec![WamInstr::SetVar {
                name: name.clone(),
                reg: reg_map[&key],
            }]
        }

        _ => todo!("{:?}", term),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compiler_bytecode::{WamInstr, WamInstrs},
        parse::query,
    };

    fn test_compile_query(source: &str, expected: Vec<WamInstr>) {
        let parsed_query = query(source).unwrap().1;
        let instructions = compile_query(parsed_query);
        assert!(
            instructions == expected,
            "Mismatch for query: {}\n\nActual:\n{:?}\nExpected:\n{:?}",
            source,
            WamInstrs(&instructions),
            WamInstrs(&expected)
        );
    }

    #[test]
    fn top_atom() {
        test_compile_query(
            "parent.",
            vec![WamInstr::PutStruct {
                functor: "parent".to_string(),
                arity: 0,
                reg: WamReg::X(0),
            }],
        );
    }

    #[test]
    fn book_example() {
        test_compile_query(
            "p(Z, h(Z,W), f(W)).",
            vec![
                WamInstr::PutVar {
                    name: "Z".to_string(),
                    reg: WamReg::X(1),
                    reg2: WamReg::X(4),
                },
                WamInstr::PutStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    reg: WamReg::X(2),
                },
                WamInstr::SetVal {
                    name: "Z".to_string(),
                    reg: WamReg::X(4),
                },
                WamInstr::SetVar {
                    name: "W".to_string(),
                    reg: WamReg::X(5),
                },
                WamInstr::PutStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(3),
                },
                WamInstr::SetVal {
                    name: "W".to_string(),
                    reg: WamReg::X(5),
                },
                WamInstr::Call {
                    predicate: "p".to_string(),
                    arity: 3,
                    to_program_counter: usize::MAX,
                },
            ],
        )
    }
    #[test]
    fn same_functor_other_arg() {
        test_compile_query(
            "p(a(X), a(Y)).",
            vec![
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    reg: WamReg::X(1),
                },
                WamInstr::SetVar {
                    name: "X".to_string(),
                    reg: WamReg::X(3),
                },
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    reg: WamReg::X(2),
                },
                WamInstr::SetVar {
                    name: "Y".to_string(),
                    reg: WamReg::X(4),
                },
                WamInstr::Call {
                    predicate: "p".to_string(),
                    arity: 2,
                    to_program_counter: usize::MAX,
                },
            ],
        );
    }

    #[test]
    fn two_heads() {
        test_compile_query(
            "p(X). q(Y).",
            vec![
                WamInstr::SetVar {
                    name: "X".to_string(),
                    reg: WamReg::X(1),
                },
                WamInstr::Call {
                    predicate: "p".to_string(),
                    arity: 1,
                    to_program_counter: usize::MAX,
                },
                WamInstr::SetVar {
                    name: "Y".to_string(),
                    reg: WamReg::X(2),
                },
                WamInstr::Call {
                    predicate: "q".to_string(),
                    arity: 1,
                    to_program_counter: usize::MAX,
                },
            ],
        );
    }
}
