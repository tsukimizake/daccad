use std::{collections::HashMap, iter::once};

use crate::{
    compiler_bytecode::{WamInstr, WamReg},
    parse::{Clause, Term},
    register_managers::ArgRegisterManager,
};

pub fn compile_db(db: Vec<Clause>) -> Vec<WamInstr> {
    let mut declared_vars = HashMap::new();
    let mut arg_register_manager = ArgRegisterManager::new();

    db.into_iter()
        .flat_map(|clause| match clause {
            Clause::Fact(term) => {
                let res = compile_db_term(term, &mut declared_vars, &mut arg_register_manager);
                declared_vars.clear();
                arg_register_manager.reset();
                res
            }
            Clause::Rule { head: _, body: _ } => {
                todo!();
            }
        })
        .collect()
}

fn compile_db_term(
    term: Term,
    declared_vars: &mut HashMap<String, WamReg>,
    arg_register_manager: &mut ArgRegisterManager,
) -> Vec<WamInstr> {
    match term {
        Term::Number(val) => {
            vec![WamInstr::UnifyNumber { val }]
        }
        Term::Atom(name) => {
            if let Some(&reg) = declared_vars.get(&name) {
                vec![WamInstr::UnifyAtom { reg }]
            } else {
                let reg = arg_register_manager.get_next();
                declared_vars.insert(name.clone(), reg);
                vec![WamInstr::GetAtom { name, reg }]
            }
        }
        Term::Var(name) => {
            if let Some(&reg) = declared_vars.get(&name) {
                vec![WamInstr::UnifyVar { reg }]
            } else {
                let reg = arg_register_manager.get_next();
                declared_vars.insert(name.clone(), reg);
                vec![WamInstr::GetVar { name, reg }]
            }
        }

        Term::TopStruct { functor, args } => {
            let head = WamInstr::Label { name: functor };
            let last = WamInstr::Proceed;

            let rest = args
                .into_iter()
                .flat_map(|arg| compile_db_term(arg, declared_vars, arg_register_manager));
            once(head).chain(rest).chain(once(last)).collect()
        }

        Term::InnerStruct { functor, args } => {
            let arity = args.len();
            let head = WamInstr::GetStruct {
                functor,
                arity,
                reg: arg_register_manager.get_next(),
            };

            let tail = args
                .into_iter()
                .flat_map(|arg| compile_db_term(arg, declared_vars, arg_register_manager));
            once(head).chain(tail).collect()
        }
        _ => {
            todo!()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_bytecode::WamReg;
    use crate::parse::clause;

    fn test_compile_db_helper(source: &str, expected: Vec<WamInstr>) {
        let parsed_clause = clause(source).unwrap().1;
        let instructions = compile_db(vec![parsed_clause]);
        assert_eq!(instructions, expected);
    }

    #[test]
    fn db_atom() {
        test_compile_db_helper(
            "parent.",
            vec![WamInstr::GetAtom {
                name: "parent".to_string(),
                reg: WamReg::A(0),
            }],
        );
    }

    #[test]
    fn db_clause() {
        test_compile_db_helper(
            "parent(john, doe).",
            vec![
                WamInstr::GetStruct {
                    functor: "parent".to_string(),
                    arity: 2,
                    reg: WamReg::A(0),
                },
                WamInstr::GetAtom {
                    name: "john".to_string(),
                    reg: WamReg::A(1),
                },
                WamInstr::GetAtom {
                    name: "doe".to_string(),
                    reg: WamReg::A(2),
                },
            ],
        );
    }

    #[test]
    fn db_clause_var_shared() {
        test_compile_db_helper(
            "a(X, X).",
            vec![
                WamInstr::GetStruct {
                    functor: "a".to_string(),
                    arity: 2,
                    reg: WamReg::A(0),
                },
                WamInstr::GetVar {
                    name: "X".to_string(),
                    reg: WamReg::A(1),
                },
                WamInstr::UnifyVar { reg: WamReg::A(1) },
            ],
        );
    }
}
