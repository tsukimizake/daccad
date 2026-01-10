use std::collections::{HashMap, HashSet, VecDeque};

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
    declared_vars: &mut HashSet<String>,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args, .. } => {
            let mut res = Vec::with_capacity(10);
            res.push(WamInstr::Label {
                name: functor.clone(),
                arity: args.len(),
            });
            let mut postponed_functors = VecDeque::with_capacity(10);
            for arg in args {
                let child_ops =
                    compile_db_term(arg, reg_map, declared_vars, &mut postponed_functors);
                res.extend(child_ops);
            }
            while let Some((_, term)) = postponed_functors.pop_front() {
                let child_ops =
                    compile_db_term(&term, reg_map, declared_vars, &mut postponed_functors);
                res.extend(child_ops);
            }

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
    declared_vars: &mut HashSet<String>,
    postponed_functors: &mut VecDeque<(WamReg, Term)>,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args, .. } => {
            let mut res = Vec::with_capacity(1 + args.len());
            res.push(WamInstr::GetStruct {
                functor: functor.clone(),
                arity: args.len(),
                reg: reg_map[&term.id()],
            });
            for arg in args {
                match arg {
                    Term::Var { name, .. } => {
                        res.push(gen_unify_var_or_val(
                            &arg.id(),
                            name,
                            reg_map,
                            declared_vars,
                        ));
                    }
                    Term::Struct { .. } => {
                        let reg = reg_map[&arg.id()];
                        res.push(WamInstr::UnifyVar {
                            name: arg.get_name().to_string(),
                            reg,
                        });
                        postponed_functors.push_back((reg, arg.clone()));
                    }
                    _ => todo!("{:?}", arg),
                }
            }
            res
        }
        Term::Var { name, .. } => vec![gen_unify_var_or_val(
            &term.id(),
            name,
            reg_map,
            declared_vars,
        )],
        _ => todo!("{:?}", term),
    }
}

fn gen_unify_var_or_val(
    term_id: &TermId,
    name: &str,
    reg_map: &HashMap<TermId, WamReg>,
    declared_vars: &mut HashSet<String>,
) -> WamInstr {
    if name != "_" && declared_vars.contains(name) {
        WamInstr::UnifyVal {
            name: name.to_string(),
            reg: reg_map[term_id],
        }
    } else {
        if name != "_" {
            declared_vars.insert(name.to_string());
        }
        WamInstr::UnifyVar {
            name: name.to_string(),
            reg: reg_map[term_id],
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
        // p(f(X), h(Y, f(a)), Y)
        // トップレベルpにはレジスタなし
        // 引数: f=X(0), h=X(1), Y=X(2), X(in f)=X(3), Y(in h)=X(4), f(in h)=X(5), a=X(6)
        test_compile_db_helper(
            "p(f(X),h(Y,f(a)), Y).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 3,
                },
                // f(X)
                WamInstr::GetStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(0),
                },
                WamInstr::UnifyVar {
                    name: "X".to_string(),
                    reg: WamReg::X(3),
                },
                // h(Y, f(a))
                WamInstr::GetStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    reg: WamReg::X(1),
                },
                WamInstr::UnifyVar {
                    name: "Y".to_string(),
                    reg: WamReg::X(4),
                },
                WamInstr::UnifyVar {
                    name: "f".to_string(),
                    reg: WamReg::X(5),
                },
                // Y (3rd arg, 2nd occurrence)
                WamInstr::UnifyVal {
                    name: "Y".to_string(),
                    reg: WamReg::X(2),
                },
                // f(a) from h's 2nd arg
                WamInstr::GetStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(5),
                },
                WamInstr::UnifyVar {
                    name: "a".to_string(),
                    reg: WamReg::X(6),
                },
                // a
                WamInstr::GetStruct {
                    functor: "a".to_string(),
                    arity: 0,
                    reg: WamReg::X(6),
                },
                WamInstr::Proceed,
            ],
        );
    }
}
