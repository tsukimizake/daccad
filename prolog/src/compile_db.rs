use std::{
    collections::{HashMap, HashSet},
    iter::once,
};

use crate::{
    compiler_bytecode::{WamInstr, WamReg},
    parse::{Clause, Term},
    register_managers::{RegKey, RegisterManager, alloc_registers, to_regkey},
};

pub fn compile_db(db: Vec<Clause>) -> Vec<WamInstr> {
    db.into_iter()
        .flat_map(|clause| match clause {
            Clause::Fact(term) => {
                let mut reg_map = HashMap::new();
                let mut reg_manager = RegisterManager::new();
                alloc_registers(&term, &mut reg_map, &mut reg_manager);
                let mut declared_vars = HashSet::new();
                let res = compile_db_term(&term, &reg_map, &mut declared_vars);
                res
            }
            Clause::Rule { head: _, body: _ } => {
                todo!();
            }
        })
        .collect()
}

fn compile_db_term(
    term: &Term,
    reg_map: &HashMap<RegKey, WamReg>,
    declared_vars: &mut HashSet<RegKey>,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args } => {
            let functor_children = args.iter().filter(|arg| {
                matches!(arg, Term::Struct { .. }) | matches!(arg, Term::Atom { .. })
            });
            let key = to_regkey(term, reg_map);
            once(WamInstr::GetStruct {
                functor: functor.clone(),
                arity: args.len(),
                reg: reg_map[&key],
            })
            .chain(args.iter().map(|functor_child| {
                let key = to_regkey(functor_child, reg_map);
                if declared_vars.contains(&key) {
                    WamInstr::UnifyVal {
                        name: functor_child.get_name().to_string(),
                        reg: reg_map[&key],
                    }
                } else {
                    declared_vars.insert(key.clone());
                    WamInstr::UnifyVar {
                        name: functor_child.get_name().to_string(),
                        reg: reg_map[&key],
                    }
                }
            }))
            .collect::<Vec<WamInstr>>()
            .into_iter()
            .chain(functor_children.flat_map(|arg| compile_db_term(arg, reg_map, declared_vars)))
            .collect()
        }
        Term::Var(name) => {
            let key = RegKey::Var(name.clone());
            if declared_vars.contains(&key) {
                vec![WamInstr::UnifyVal {
                    name: name.clone(),
                    reg: reg_map[&key],
                }]
            } else {
                declared_vars.insert(key.clone());
                vec![WamInstr::UnifyVar {
                    name: name.clone(),
                    reg: reg_map[&key],
                }]
            }
        }
        Term::Atom(name) => {
            let key = RegKey::Var(name.clone());
            if declared_vars.contains(&key) {
                vec![WamInstr::GetStruct {
                    functor: name.clone(),
                    arity: 0,
                    reg: reg_map[&key],
                }]
            } else {
                declared_vars.insert(key.clone());
                vec![WamInstr::GetStruct {
                    functor: name.clone(),
                    arity: 0,
                    reg: reg_map[&key],
                }]
            }
        }
        _ => {
            todo!("{:?}", term)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_bytecode::WamReg;
    use crate::parse::database;

    fn test_compile_db_helper(source: &str, expected: Vec<WamInstr>) {
        let parsed = database(source).unwrap();
        let instructions = compile_db(parsed.clone());
        assert_eq!(instructions, expected);
    }

    #[test]
    fn sample_code() {
        test_compile_db_helper(
            "p(f(X),h(Y,f(a)), Y).",
            vec![
                WamInstr::GetStruct {
                    functor: "p".to_string(),
                    arity: 3,
                    reg: WamReg::X(0),
                },
                WamInstr::UnifyVar {
                    name: "f".to_string(),
                    reg: WamReg::X(1),
                },
                WamInstr::UnifyVar {
                    name: "h".to_string(),
                    reg: WamReg::X(3),
                },
                WamInstr::UnifyVar {
                    name: "Y".to_string(),
                    reg: WamReg::X(4),
                },
                WamInstr::GetStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(1),
                },
                WamInstr::UnifyVar {
                    name: "X".to_string(),
                    reg: WamReg::X(2),
                },
                WamInstr::GetStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    reg: WamReg::X(3),
                },
                WamInstr::UnifyVal {
                    name: "Y".to_string(),
                    reg: WamReg::X(4),
                },
                WamInstr::UnifyVar {
                    name: "f".to_string(),
                    reg: WamReg::X(5),
                },
                WamInstr::GetStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(5),
                },
                WamInstr::UnifyVar {
                    name: "a".to_string(),
                    reg: WamReg::X(6),
                },
                WamInstr::GetStruct {
                    functor: "a".to_string(),
                    arity: 0,
                    reg: WamReg::X(6),
                },
            ],
        );
    }
}
