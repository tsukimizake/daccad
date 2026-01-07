use std::{
    collections::{HashMap, HashSet},
    iter::once,
};

use crate::{
    compiler_bytecode::{WamInstr, WamReg},
    parse::{Clause, Term, TermId},
    register_managers::{RegisterManager, alloc_registers},
};

pub fn compile_db(db: Vec<Clause>) -> Vec<WamInstr> {
    db.into_iter()
        .flat_map(|clause| match clause {
            Clause::Fact(term) => {
                let mut reg_map = HashMap::new();
                let mut reg_manager = RegisterManager::new();
                alloc_registers(&term, &mut reg_map, &mut reg_manager);
                println!("term: {:?}", term);
                println!("reg_map: {:?}", reg_map);
                let mut declared_vars = HashSet::new();
                compile_db_term_top(&term, &reg_map, &mut declared_vars)
            }
            Clause::Rule { head: _, body: _ } => {
                todo!();
            }
        })
        .collect()
}

fn compile_db_term_top(
    term: &Term,
    reg_map: &HashMap<TermId, WamReg>,
    declared_vars: &mut HashSet<TermId>,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args, .. } => {
            let mut res = Vec::with_capacity(10);
            res.push(WamInstr::Label {
                name: functor.clone(),
                arity: args.len(),
            });
            args.iter().for_each(|arg| {
                let mut postponed_functors = Vec::with_capacity(10);
                let child_ops =
                    compile_db_term(arg, reg_map, declared_vars, &mut postponed_functors);
                res.extend(child_ops);
            });

            res.push(WamInstr::Proceed);
            res
        }
        _ => {
            todo!("{:?}", term)
        }
    }
}

fn compile_db_term(
    term: &Term,
    reg_map: &HashMap<TermId, WamReg>,
    declared_vars: &mut HashSet<TermId>,
    postponed_functors: &mut Vec<(WamReg, Term)>,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args, .. } => {
            let key = term.id();
            once(WamInstr::GetStruct {
                functor: functor.clone(),
                arity: args.len(),
                reg: reg_map[&key],
            })
            .chain(args.into_iter().map(|functor_child| {
                let child_name = functor_child.get_name().to_string();
                let key = functor_child.id();
                match functor_child {
                    Term::Struct { .. } => {
                        postponed_functors
                            .push((reg_map[&functor_child.id()], functor_child.clone()));
                        gen_unify_var_or_val(&key, &child_name, reg_map, declared_vars)
                    }
                    Term::Var { .. } => {
                        gen_unify_var_or_val(&key, &child_name, reg_map, declared_vars)
                    }
                    _ => {
                        todo!("{:?}", functor_child)
                    }
                }
            }))
            .collect::<Vec<WamInstr>>()
            .into_iter()
            .chain(compile_db_term_struct_child(
                reg_map,
                declared_vars,
                postponed_functors,
            ))
            .collect()
        }
        Term::Var { name, id } => vec![gen_unify_var_or_val(id, name, reg_map, declared_vars)],
        _ => {
            todo!("{:?}", term)
        }
    }
}

fn gen_unify_var_or_val(
    term_id: &TermId,
    name: &str,
    reg_map: &HashMap<TermId, WamReg>,
    declared_vars: &mut HashSet<TermId>,
) -> WamInstr {
    if declared_vars.contains(term_id) {
        WamInstr::UnifyVal {
            name: name.to_string(),
            reg: reg_map[term_id],
        }
    } else {
        declared_vars.insert(term_id.clone());
        WamInstr::UnifyVar {
            name: name.to_string(),
            reg: reg_map[term_id],
        }
    }
}

fn compile_db_term_struct_child(
    child: &Term,
    reg_map: &HashMap<TermId, WamReg>,
    declared_vars: &mut HashSet<TermId>,
    postponed_functors: &mut Vec<(WamReg, Term)>,
) -> Vec<WamInstr> {
    match child {
        Term::Struct { .. } => {
            postponed_functors.push((reg_map[&child.id()], child.clone()));
            if declared_vars.contains(&child.id()) {
                vec![WamInstr::UnifyVal {
                    name: child.get_name().to_string(),
                    reg: reg_map[&child.id()],
                }]
            } else {
                declared_vars.insert(child.id().clone());
                vec![WamInstr::UnifyVar {
                    name: child.get_name().to_string(),
                    reg: reg_map[&child.id()],
                }]
            }
        }
        Term::Var { name, id } => {
            if declared_vars.contains(id) {
                vec![WamInstr::UnifyVal {
                    name: name.clone(),
                    reg: reg_map[id],
                }]
            } else {
                declared_vars.insert(id.clone());
                vec![WamInstr::UnifyVar {
                    name: name.clone(),
                    reg: reg_map[&id],
                }]
            }
        }
        _ => {
            todo!("{:?}", child)
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
            // => p(f(X), h(Y, X6), Y), X6 = f(X7), X7 = a, X8=Y.
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 3,
                },
                // f(X)
                WamInstr::GetStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(1),
                },
                WamInstr::UnifyVar {
                    name: "X".to_string(),
                    reg: WamReg::X(4),
                },
                // h(Y, X6)
                WamInstr::GetStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    reg: WamReg::X(2),
                },
                WamInstr::UnifyVar {
                    name: "Y".to_string(),
                    reg: WamReg::X(5),
                },
                WamInstr::UnifyVar {
                    name: "f".to_string(),
                    reg: WamReg::X(6),
                },
                // Y
                WamInstr::UnifyVal {
                    name: "Y".to_string(),
                    reg: WamReg::X(3), // 5?
                },
                // X6 = f(X7)
                WamInstr::GetStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(6),
                },
                WamInstr::UnifyVar {
                    name: "a".to_string(),
                    reg: WamReg::X(7),
                },
                // X7 = a
                WamInstr::GetStruct {
                    functor: "a".to_string(),
                    arity: 0,
                    reg: WamReg::X(7),
                },
                WamInstr::Proceed,
            ],
        );
    }
}
