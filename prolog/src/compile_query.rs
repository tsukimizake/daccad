use std::collections::HashMap;

use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::parse::Term;
use crate::register_managers::RegisterManager;

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
    todo!()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RegKey {
    Functor {
        name: String,
        arity: usize,
        args: Vec<WamReg>,
    },
    Var(String),
}

fn alloc_registers(
    term: &Term,
    declared_vars: &mut HashMap<RegKey, WamReg>,
    reg_manager: &mut RegisterManager,
) -> WamReg {
    match term {
        Term::TopStruct { functor, args } => {
            let reg = reg_manager.get_next();
            let arg_keys = args
                .iter()
                .map(|arg| alloc_registers(arg, declared_vars, reg_manager))
                .collect();
            let f = RegKey::Functor {
                name: functor.clone(),
                arity: args.len(),
                args: arg_keys,
            };
            declared_vars.insert(f.clone(), reg);
            reg
        }

        Term::InnerStruct { functor, args } => {
            let reg = reg_manager.get_next();
            let arg_keys = args
                .iter()
                .map(|arg| alloc_registers(arg, declared_vars, reg_manager))
                .collect();
            let f = RegKey::Functor {
                name: functor.clone(),
                arity: args.len(),
                args: arg_keys,
            };
            declared_vars.insert(f.clone(), reg);
            reg
        }

        Term::InnerAtom(name) => {
            let k = RegKey::Var(name.clone());
            if let Some(&reg) = declared_vars.get(&k) {
                reg
            } else {
                let reg = reg_manager.get_next();
                declared_vars.insert(k.clone(), reg);
                reg
            }
        }
        Term::Var(name) => {
            let k = RegKey::Var(name.clone());
            if let Some(&reg) = declared_vars.get(&k) {
                reg
            } else {
                let reg = reg_manager.get_next();
                declared_vars.insert(k.clone(), reg);
                reg
            }
        }
        _ => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compiler_bytecode::WamInstr, parse::query};

    fn test_alloc_registers_helper(source: &str, expected: HashMap<RegKey, WamReg>) {
        let parsed_query = query(source).unwrap().1;
        let term = &parsed_query[0];
        let mut declared_vars = HashMap::new();
        let mut reg_manager = RegisterManager::new();
        let _ = alloc_registers(term, &mut declared_vars, &mut reg_manager);
        assert_eq!(declared_vars, expected);
    }

    #[test]
    fn test_alloc_registers() {
        test_alloc_registers_helper("p(Z, h(Z,W), f(W)).", {
            let mut map = HashMap::new();
            map.insert(
                RegKey::Functor {
                    name: "p".to_string(),
                    arity: 3,
                    args: vec![WamReg::X(1), WamReg::X(2), WamReg::X(4)],
                },
                WamReg::X(0),
            );
            map.insert(RegKey::Var("Z".to_string()), WamReg::X(1));
            map.insert(
                RegKey::Functor {
                    name: "h".to_string(),
                    arity: 2,
                    args: vec![WamReg::X(1), WamReg::X(3)],
                },
                WamReg::X(2),
            );
            map.insert(RegKey::Var("W".to_string()), WamReg::X(3));
            map.insert(
                RegKey::Functor {
                    name: "f".to_string(),
                    arity: 1,
                    args: vec![WamReg::X(3)],
                },
                WamReg::X(4),
            );
            map
        });
    }

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

    #[test]
    fn query_example() {
        test_compile_query_helper(
            "p(Z, h(Z,W), f(W)).",
            vec![
                WamInstr::PutStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    reg: WamReg::X(3),
                },
                WamInstr::SetVar { reg: WamReg::X(2) },
                WamInstr::SetVar { reg: WamReg::X(5) },
                WamInstr::PutStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(4),
                },
                WamInstr::SetVal { reg: WamReg::X(5) },
                WamInstr::PutStruct {
                    functor: "p".to_string(),
                    arity: 3,
                    reg: WamReg::X(1),
                },
                WamInstr::SetVal { reg: WamReg::X(5) },
                WamInstr::SetVal { reg: WamReg::X(3) },
                WamInstr::SetVal { reg: WamReg::X(4) },
            ],
        )
    }
}
