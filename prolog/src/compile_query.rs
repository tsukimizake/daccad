use std::collections::HashMap;
use std::iter::once;

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
    compile_defs(&term, &declared_vars)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RegKey {
    TopFunctor {
        name: String,
        arity: usize,
        args: Vec<WamReg>,
    },
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
            let f = RegKey::TopFunctor {
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
        Term::TopAtom(name) => {
            let k = RegKey::Var(name.clone());
            if let Some(&reg) = declared_vars.get(&k) {
                reg
            } else {
                let reg = reg_manager.get_next();
                declared_vars.insert(k.clone(), reg);
                reg
            }
        }
        _ => todo!("{:?}", term),
    }
}

fn compile_defs(term: &Term, reg_map: &HashMap<RegKey, WamReg>) -> Vec<WamInstr> {
    match term {
        Term::TopStruct { functor, args } => {
            let functor_children = args
                .iter()
                .filter(|arg| matches!(arg, Term::InnerStruct { .. }))
                .flat_map(|arg| compile_defs(arg, reg_map));

            let key = to_regkey(term, reg_map);

            functor_children
                .chain(once(WamInstr::PutStruct {
                    functor: functor.clone(),
                    arity: args.len(),
                    reg: reg_map[&key],
                }))
                .chain(args.iter().map(|arg| {
                    let reg = reg_map[&to_regkey(arg, reg_map)];
                    WamInstr::SetVal {
                        name: arg.get_name().to_string(),
                        reg,
                    }
                }))
                .collect()
        }
        Term::InnerStruct { functor, args } => {
            let functor_children = args
                .iter()
                .filter(|arg| matches!(arg, Term::InnerStruct { .. }))
                .flat_map(|arg| compile_defs(arg, reg_map));

            let key = to_regkey(term, reg_map);

            functor_children
                .chain(once(WamInstr::PutStruct {
                    functor: functor.clone(),
                    arity: args.len(),
                    reg: reg_map[&key],
                }))
                .chain(args.iter().map(|arg| {
                    let reg = reg_map[&to_regkey(arg, reg_map)];
                    WamInstr::SetVal {
                        name: arg.get_name().to_string(),
                        reg,
                    }
                }))
                .collect()
        }
        Term::InnerAtom(name) => {
            let key = to_regkey(term, reg_map);
            vec![WamInstr::SetVar {
                name: name.clone(),
                reg: reg_map[&key],
            }]
        }
        Term::Var(name) => {
            let key = to_regkey(term, reg_map);
            vec![WamInstr::SetVar {
                name: name.clone(),
                reg: reg_map[&key],
            }]
        }
        Term::TopAtom(name) => {
            let key = to_regkey(term, reg_map);
            vec![WamInstr::SetVar {
                name: name.clone(),
                reg: reg_map[&key],
            }]
        }

        _ => todo!("{:?}", term),
    }
}

fn to_regkey(term: &Term, reg_map: &HashMap<RegKey, WamReg>) -> RegKey {
    match term {
        Term::TopStruct { functor, args } => RegKey::TopFunctor {
            name: functor.clone(),
            arity: args.len(),
            args: args
                .iter()
                .map(|arg| to_regkey(arg, reg_map))
                .map(|k| reg_map[&k])
                .collect(),
        },
        Term::InnerStruct { functor, args } => RegKey::Functor {
            name: functor.clone(),
            arity: args.len(),
            args: args
                .iter()
                .map(|arg| to_regkey(arg, reg_map))
                .map(|k| reg_map[&k])
                .collect(),
        },
        Term::InnerAtom(name) | Term::Var(name) | Term::TopAtom(name) => RegKey::Var(name.clone()),
        _ => panic!("Unsupported term for RegKey: {:?}", term),
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
                RegKey::TopFunctor {
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
    fn top_atom() {
        test_compile_query_helper(
            "parent.",
            vec![WamInstr::SetVar {
                name: "parent".to_string(),
                reg: WamReg::X(0),
            }],
        );
    }

    #[test]
    fn book_example() {
        test_compile_query_helper(
            "p(Z, h(Z,W), f(W)).",
            vec![
                WamInstr::PutStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    reg: WamReg::X(2),
                },
                WamInstr::SetVal {
                    name: "Z".to_string(),
                    reg: WamReg::X(1),
                },
                WamInstr::SetVal {
                    name: "W".to_string(),
                    reg: WamReg::X(3),
                },
                WamInstr::PutStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(4),
                },
                WamInstr::SetVal {
                    name: "W".to_string(),
                    reg: WamReg::X(3),
                },
                WamInstr::PutStruct {
                    functor: "p".to_string(),
                    arity: 3,
                    reg: WamReg::X(0),
                },
                WamInstr::SetVal {
                    name: "Z".to_string(),
                    reg: WamReg::X(1),
                },
                WamInstr::SetVal {
                    name: "h".to_string(),
                    reg: WamReg::X(2),
                },
                WamInstr::SetVal {
                    name: "f".to_string(),
                    reg: WamReg::X(4),
                },
            ],
        )
    }
    #[test]
    fn same_functor_other_arg() {
        test_compile_query_helper(
            "p(a(X), a(Y)).",
            vec![
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    reg: WamReg::X(1),
                },
                WamInstr::SetVal {
                    name: "X".to_string(),
                    reg: WamReg::X(2),
                },
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    reg: WamReg::X(3),
                },
                WamInstr::SetVal {
                    name: "Y".to_string(),
                    reg: WamReg::X(4),
                },
                WamInstr::PutStruct {
                    functor: "p".to_string(),
                    arity: 2,
                    reg: WamReg::X(0),
                },
                WamInstr::SetVal {
                    name: "a".to_string(),
                    reg: WamReg::X(1),
                },
                WamInstr::SetVal {
                    name: "a".to_string(),
                    reg: WamReg::X(3),
                },
            ],
        );
    }
}
