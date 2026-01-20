use std::collections::{HashMap, VecDeque};

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
                let mut declared_vars = HashMap::new();
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
    declared_vars: &mut HashMap<String, WamReg>,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args, .. } => {
            let mut res = Vec::with_capacity(10);
            res.push(WamInstr::Label {
                name: functor.clone(),
                arity: args.len(),
            });
            let mut postponed_functors = VecDeque::with_capacity(10);
            // 恒久変数レジスタは引数の数から始まる
            let mut perm_reg_counter = args.len();
            for (arg_index, arg) in args.iter().enumerate() {
                let child_ops = compile_db_term_toplevel_arg(
                    arg,
                    arg_index,
                    reg_map,
                    declared_vars,
                    &mut postponed_functors,
                    &mut perm_reg_counter,
                );
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

fn compile_db_term_toplevel_arg(
    term: &Term,
    arg_index: usize,
    reg_map: &HashMap<TermId, WamReg>,
    declared_vars: &mut HashMap<String, WamReg>,
    postponed_functors: &mut VecDeque<(WamReg, Term)>,
    perm_reg_counter: &mut usize,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { .. } => {
            // Structの場合は従来通りcompile_db_termを使う
            compile_db_term(term, reg_map, declared_vars, postponed_functors)
        }
        Term::Var { name, .. } => {
            // トップレベルの変数引数にはGetVar/GetValを使う
            // reg は引数レジスタ (X0, X1, ...)
            let reg = WamReg::X(arg_index);
            if name != "_" && declared_vars.contains_key(name) {
                vec![WamInstr::GetVal {
                    name: name.to_string(),
                    with: declared_vars[name],
                    reg,
                }]
            } else {
                // with は恒久変数レジスタ (引数の数から始まる)
                let with = WamReg::X(*perm_reg_counter);
                *perm_reg_counter += 1;
                if name != "_" {
                    declared_vars.insert(name.to_string(), with);
                }
                vec![WamInstr::GetVar {
                    name: name.to_string(),
                    with,
                    reg,
                }]
            }
        }
        _ => todo!("{:?}", term),
    }
}

fn compile_db_term(
    term: &Term,
    reg_map: &HashMap<TermId, WamReg>,
    declared_vars: &mut HashMap<String, WamReg>,
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
    declared_vars: &mut HashMap<String, WamReg>,
) -> WamInstr {
    let reg = reg_map[term_id];
    if name != "_" && declared_vars.contains_key(name) {
        WamInstr::UnifyVal {
            name: name.to_string(),
            reg,
        }
    } else {
        if name != "_" {
            declared_vars.insert(name.to_string(), reg);
        }
        WamInstr::UnifyVar {
            name: name.to_string(),
            reg,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_bytecode::{WamInstrs, WamReg};
    use crate::parse::database;

    fn test_compile_db_helper(source: &str, expected: Vec<WamInstr>) {
        let parsed = database(source).unwrap();
        let instructions = compile_db(parsed.clone());
        assert!(
            instructions == expected,
            "Mismatch for db: {}\n\nActual:\n{:?}\nExpected:\n{:?}",
            source,
            WamInstrs(&instructions),
            WamInstrs(&expected)
        );
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
                WamInstr::GetVal {
                    name: "Y".to_string(),
                    reg: WamReg::X(2),
                    with: WamReg::X(4),
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

    #[test]
    fn toplevel_vars_xxy() {
        test_compile_db_helper(
            "honi(X, X, Y).",
            vec![
                WamInstr::Label {
                    name: "honi".to_string(),
                    arity: 3,
                },
                // X (1st occurrence)
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::X(3),
                    reg: WamReg::X(0),
                },
                // X (2nd occurrence)
                WamInstr::GetVal {
                    name: "X".to_string(),
                    with: WamReg::X(3),
                    reg: WamReg::X(1),
                },
                // Y (1st occurrence)
                WamInstr::GetVar {
                    name: "Y".to_string(),
                    with: WamReg::X(4),
                    reg: WamReg::X(2),
                },
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn toplevel_vars_xyxy() {
        test_compile_db_helper(
            "honi(X, Y, X, Y).",
            vec![
                WamInstr::Label {
                    name: "honi".to_string(),
                    arity: 4,
                },
                // X (1st occurrence)
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::X(4),
                    reg: WamReg::X(0),
                },
                // Y (1st occurrence)
                WamInstr::GetVar {
                    name: "Y".to_string(),
                    with: WamReg::X(5),
                    reg: WamReg::X(1),
                },
                // X (2nd occurrence)
                WamInstr::GetVal {
                    name: "X".to_string(),
                    with: WamReg::X(4),
                    reg: WamReg::X(2),
                },
                // Y (2nd occurrence)
                WamInstr::GetVal {
                    name: "Y".to_string(),
                    with: WamReg::X(5),
                    reg: WamReg::X(3),
                },
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn sample_rule() {
        test_compile_db_helper(
            "p(X,Y) :- q(X, Z), r(Z, Y).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 2,
                },
                WamInstr::Allocate { size: 2 },
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::X(2),
                    reg: WamReg::X(0),
                },
                WamInstr::GetVar {
                    name: "Y".to_string(),
                    with: WamReg::X(3),
                    reg: WamReg::X(1),
                },
                WamInstr::PutVal {
                    name: "X".to_string(),
                    reg: WamReg::X(0),
                    with: WamReg::X(2),
                },
                WamInstr::PutVar {
                    name: "Z".to_string(),
                    reg: WamReg::X(0),
                    with: WamReg::X(4),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 2,
                },
                WamInstr::PutVal {
                    name: "Z".to_string(),
                    reg: WamReg::X(0),
                    with: WamReg::X(4),
                },
                WamInstr::PutVal {
                    name: "Y".to_string(),
                    reg: WamReg::X(1),
                    with: WamReg::X(3),
                },
                WamInstr::CallTemp {
                    predicate: "r".to_string(),
                    arity: 2,
                },
                WamInstr::Deallocate,
            ],
        );
    }
}
