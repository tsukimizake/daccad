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
            Clause::Rule { head, body } => {
                let mut reg_map = HashMap::new();
                let mut reg_manager = RegisterManager::new();
                alloc_registers(&head, &mut reg_map, &mut reg_manager);
                for goal in &body {
                    alloc_registers(goal, &mut reg_map, &mut reg_manager);
                }
                let mut declared_vars = HashMap::new();
                compile_rule(&head, &body, &reg_map, &mut declared_vars)
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

fn compile_rule(
    head: &Term,
    body: &[Term],
    _reg_map: &HashMap<TermId, WamReg>,
    declared_vars: &mut HashMap<String, WamReg>,
) -> Vec<WamInstr> {
    match head {
        Term::Struct { functor, args, .. } => {
            let mut res = Vec::with_capacity(20);
            res.push(WamInstr::Label {
                name: functor.clone(),
                arity: args.len(),
            });

            // bodyの複数のgoalにまたがって出現する変数を取得
            let cross_goal_vars = get_cross_goal_vars(body);
            res.push(WamInstr::Allocate {
                size: cross_goal_vars.len(),
            });

            // head引数をGetVar/GetValで処理
            // cross-goal変数のみYレジスタに配置
            let mut perm_reg_counter = 0;
            let mut temp_reg_counter = args.len(); // Xレジスタは引数の後から
            for (arg_index, arg) in args.iter().enumerate() {
                let reg = WamReg::X(arg_index);
                match arg {
                    Term::Var { name, .. } => {
                        if name != "_" && declared_vars.contains_key(name) {
                            res.push(WamInstr::GetVal {
                                name: name.to_string(),
                                with: declared_vars[name],
                                reg,
                            });
                        } else {
                            // cross-goal変数ならYレジスタ、そうでなければXレジスタ
                            let with = if cross_goal_vars.contains(name) {
                                let w = WamReg::Y(perm_reg_counter);
                                perm_reg_counter += 1;
                                w
                            } else {
                                let w = WamReg::X(temp_reg_counter);
                                temp_reg_counter += 1;
                                w
                            };
                            if name != "_" {
                                declared_vars.insert(name.to_string(), with);
                            }
                            res.push(WamInstr::GetVar {
                                name: name.to_string(),
                                with,
                                reg,
                            });
                        }
                    }
                    Term::Struct { .. } => {
                        compile_head_struct_arg(
                            arg,
                            reg,
                            declared_vars,
                            &cross_goal_vars,
                            &mut perm_reg_counter,
                            &mut temp_reg_counter,
                            &mut res,
                        );
                    }
                    _ => todo!("{:?}", arg),
                }
            }

            // bodyの各ゴールをコンパイル
            for goal in body {
                res.extend(compile_body_goal(
                    goal,
                    declared_vars,
                    &cross_goal_vars,
                    &mut perm_reg_counter,
                    &mut temp_reg_counter,
                ));
            }

            res.push(WamInstr::Deallocate);
            res.push(WamInstr::Proceed);
            res
        }
        _ => todo!("{:?}", head),
    }
}

fn compile_head_struct_arg(
    term: &Term,
    reg: WamReg,
    declared_vars: &mut HashMap<String, WamReg>,
    cross_goal_vars: &std::collections::HashSet<String>,
    perm_reg_counter: &mut usize,
    temp_reg_counter: &mut usize,
    res: &mut Vec<WamInstr>,
) {
    if let Term::Struct { functor, args, .. } = term {
        res.push(WamInstr::GetStruct {
            functor: functor.clone(),
            arity: args.len(),
            reg,
        });

        // ネストした構造体を先に処理するため、構造体引数のレジスタを予約
        let mut nested_structs = Vec::new();
        for inner_arg in args {
            if matches!(inner_arg, Term::Struct { .. }) {
                let nested_reg = WamReg::X(*temp_reg_counter);
                *temp_reg_counter += 1;
                nested_structs.push((inner_arg, nested_reg));
            }
        }

        // UnifyVar/UnifyVal を発行
        let mut nested_idx = 0;
        for inner_arg in args {
            match inner_arg {
                Term::Var { name, .. } => {
                    if name != "_" && declared_vars.contains_key(name) {
                        res.push(WamInstr::UnifyVal {
                            name: name.to_string(),
                            reg: declared_vars[name],
                        });
                    } else {
                        let with = if cross_goal_vars.contains(name) {
                            let w = WamReg::Y(*perm_reg_counter);
                            *perm_reg_counter += 1;
                            w
                        } else {
                            let w = WamReg::X(*temp_reg_counter);
                            *temp_reg_counter += 1;
                            w
                        };
                        if name != "_" {
                            declared_vars.insert(name.to_string(), with);
                        }
                        res.push(WamInstr::UnifyVar {
                            name: name.to_string(),
                            reg: with,
                        });
                    }
                }
                Term::Struct { .. } => {
                    let (_, nested_reg) = nested_structs[nested_idx];
                    nested_idx += 1;
                    res.push(WamInstr::UnifyVar {
                        name: inner_arg.get_name().to_string(),
                        reg: nested_reg,
                    });
                }
                _ => todo!("{:?}", inner_arg),
            }
        }

        for (nested_term, nested_reg) in nested_structs {
            compile_head_struct_arg(
                nested_term,
                nested_reg,
                declared_vars,
                cross_goal_vars,
                perm_reg_counter,
                temp_reg_counter,
                res,
            );
        }
    }
}

/// bodyの複数のgoalにまたがって出現する変数の集合を返す
fn get_cross_goal_vars(body: &[Term]) -> std::collections::HashSet<String> {
    use std::collections::{HashMap, HashSet};
    let mut var_goal_count: HashMap<String, usize> = HashMap::new();

    // bodyの各goalをカウント
    for goal in body {
        let mut goal_vars = HashSet::new();
        collect_var_names(goal, &mut goal_vars);
        for var in goal_vars {
            *var_goal_count.entry(var).or_insert(0) += 1;
        }
    }

    var_goal_count
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(name, _)| name)
        .collect()
}

fn collect_var_names(term: &Term, result: &mut std::collections::HashSet<String>) {
    match term {
        Term::Var { name, .. } => {
            if name != "_" {
                result.insert(name.clone());
            }
        }
        Term::Struct { args, .. } => {
            for arg in args {
                collect_var_names(arg, result);
            }
        }
        _ => {}
    }
}

fn compile_body_goal(
    goal: &Term,
    declared_vars: &mut HashMap<String, WamReg>,
    cross_goal_vars: &std::collections::HashSet<String>,
    perm_reg_counter: &mut usize,
    temp_reg_counter: &mut usize,
) -> Vec<WamInstr> {
    match goal {
        Term::Struct { functor, args, .. } => {
            let mut res = Vec::new();

            // 各引数をPutVar/PutValで引数レジスタにセット
            // cross-goal変数のみYレジスタに配置
            for (arg_index, arg) in args.iter().enumerate() {
                let arg_reg = WamReg::X(arg_index);
                match arg {
                    Term::Var { name, .. } => {
                        if name != "_" && declared_vars.contains_key(name) {
                            res.push(WamInstr::PutVal {
                                name: name.to_string(),
                                arg_reg,
                                with: declared_vars[name],
                            });
                        } else {
                            // cross-goal変数ならYレジスタ、そうでなければXレジスタ
                            let with = if cross_goal_vars.contains(name) {
                                let w = WamReg::Y(*perm_reg_counter);
                                *perm_reg_counter += 1;
                                w
                            } else {
                                let w = WamReg::X(*temp_reg_counter);
                                *temp_reg_counter += 1;
                                w
                            };
                            if name != "_" {
                                declared_vars.insert(name.to_string(), with);
                            }
                            res.push(WamInstr::PutVar {
                                name: name.to_string(),
                                arg_reg,
                                with,
                            });
                        }
                    }
                    _ => todo!("{:?}", arg),
                }
            }

            res.push(WamInstr::CallTemp {
                predicate: functor.clone(),
                arity: args.len(),
            });

            res
        }
        _ => todo!("{:?}", goal),
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
        // p(X,Y) :- q(X, Z), r(Z, Y).
        // X: headとq(X,Z)に出現 → bodyでは1つのgoalのみ → Xレジスタ
        // Y: headとr(Z,Y)に出現 → bodyでは1つのgoalのみ → Xレジスタ
        // Z: q(X,Z)とr(Z,Y)に出現 → bodyで複数のgoalにまたがる → Yレジスタ
        test_compile_db_helper(
            "p(X,Y) :- q(X, Z), r(Z, Y).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 2,
                },
                WamInstr::Allocate { size: 1 }, // Zのみ
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
                    arg_reg: WamReg::X(0),
                    with: WamReg::X(2),
                },
                WamInstr::PutVar {
                    name: "Z".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::Y(0),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 2,
                },
                WamInstr::PutVal {
                    name: "Z".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::PutVal {
                    name: "Y".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::X(3),
                },
                WamInstr::CallTemp {
                    predicate: "r".to_string(),
                    arity: 2,
                },
                WamInstr::Deallocate,
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn single_goal_rule_no_y_register() {
        // p(X) :- q(X).
        // Xはheadとbodyのゴールにあるがbodyはgoalが1つのみなのでXレジスタ
        // Allocate size: 0 (Yレジスタ不要)
        test_compile_db_helper(
            "p(X) :- q(X).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 1,
                },
                WamInstr::Allocate { size: 0 },
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::X(1),
                    reg: WamReg::X(0),
                },
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::X(1),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 1,
                },
                WamInstr::Deallocate,
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn multiple_cross_goal_vars() {
        // p(X, Y) :- q(X, Y), r(X, Y).
        // XとYの両方がbodyの複数goalにまたがる → 両方Yレジスタ
        test_compile_db_helper(
            "p(X, Y) :- q(X, Y), r(X, Y).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 2,
                },
                WamInstr::Allocate { size: 2 }, // XとY
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::Y(0),
                    reg: WamReg::X(0),
                },
                WamInstr::GetVar {
                    name: "Y".to_string(),
                    with: WamReg::Y(1),
                    reg: WamReg::X(1),
                },
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::PutVal {
                    name: "Y".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::Y(1),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 2,
                },
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::PutVal {
                    name: "Y".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::Y(1),
                },
                WamInstr::CallTemp {
                    predicate: "r".to_string(),
                    arity: 2,
                },
                WamInstr::Deallocate,
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn head_var_not_in_body() {
        // p(X, Y) :- q(X).
        // Xはheadとbodyにあるが、bodyは1 goalなのでXレジスタ
        // Yはheadにしかないので、Xレジスタ（cross-goal変数ではない）
        test_compile_db_helper(
            "p(X, Y) :- q(X).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 2,
                },
                WamInstr::Allocate { size: 0 }, // cross-goal変数なし
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
                    arg_reg: WamReg::X(0),
                    with: WamReg::X(2),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 1,
                },
                WamInstr::Deallocate,
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn anonymous_var_in_rule() {
        test_compile_db_helper(
            "p(X, _) :- q(X, _).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 2,
                },
                WamInstr::Allocate { size: 0 },
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::X(2),
                    reg: WamReg::X(0),
                },
                WamInstr::GetVar {
                    name: "_".to_string(),
                    with: WamReg::X(3),
                    reg: WamReg::X(1),
                },
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::X(2),
                },
                WamInstr::PutVar {
                    name: "_".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::X(4),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 2,
                },
                WamInstr::Deallocate,
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn three_goals_chain() {
        // p(X, Y, Z) :- q(X, A), r(A, B), s(B, Z).
        // X: qのみ → Xレジスタ
        // Y: headのみ → Xレジスタ
        // Z: sのみ → Xレジスタ
        // A: q, r → Yレジスタ (cross-goal)
        // B: r, s → Yレジスタ (cross-goal)
        test_compile_db_helper(
            "p(X, Y, Z) :- q(X, A), r(A, B), s(B, Z).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 3,
                },
                WamInstr::Allocate { size: 2 }, // AとB
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::X(3),
                    reg: WamReg::X(0),
                },
                WamInstr::GetVar {
                    name: "Y".to_string(),
                    with: WamReg::X(4),
                    reg: WamReg::X(1),
                },
                WamInstr::GetVar {
                    name: "Z".to_string(),
                    with: WamReg::X(5),
                    reg: WamReg::X(2),
                },
                // q(X, A)
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::X(3),
                },
                WamInstr::PutVar {
                    name: "A".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::Y(0),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 2,
                },
                // r(A, B)
                WamInstr::PutVal {
                    name: "A".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::PutVar {
                    name: "B".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::Y(1),
                },
                WamInstr::CallTemp {
                    predicate: "r".to_string(),
                    arity: 2,
                },
                // s(B, Z)
                WamInstr::PutVal {
                    name: "B".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(1),
                },
                WamInstr::PutVal {
                    name: "Z".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::X(5),
                },
                WamInstr::CallTemp {
                    predicate: "s".to_string(),
                    arity: 2,
                },
                WamInstr::Deallocate,
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn head_var_used_across_all_goals() {
        // append([], L, L) :- ...
        // 簡易版: p(X) :- q(X), r(X), s(X).
        // Xはすべてのgoalにまたがる → Yレジスタ
        test_compile_db_helper(
            "p(X) :- q(X), r(X), s(X).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 1,
                },
                WamInstr::Allocate { size: 1 }, // X
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::Y(0),
                    reg: WamReg::X(0),
                },
                // q(X)
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 1,
                },
                // r(X)
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::CallTemp {
                    predicate: "r".to_string(),
                    arity: 1,
                },
                // s(X)
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::CallTemp {
                    predicate: "s".to_string(),
                    arity: 1,
                },
                WamInstr::Deallocate,
                WamInstr::Proceed,
            ],
        );
    }

    #[test]
    fn local_var_single_goal() {
        // p(X) :- q(X, Y).
        // Xはheadとbodyにあるがbodyは1goalなのでXレジスタ
        // Yはbodyのみでかつ1goalなのでXレジスタ
        test_compile_db_helper(
            "p(X) :- q(X, Y).",
            vec![
                WamInstr::Label {
                    name: "p".to_string(),
                    arity: 1,
                },
                WamInstr::Allocate { size: 0 }, // cross-goal変数なし
                WamInstr::GetVar {
                    name: "X".to_string(),
                    with: WamReg::X(1),
                    reg: WamReg::X(0),
                },
                WamInstr::PutVal {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::X(1),
                },
                WamInstr::PutVar {
                    name: "Y".to_string(),
                    arg_reg: WamReg::X(1),
                    with: WamReg::X(2),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 2,
                },
                WamInstr::Deallocate,
                WamInstr::Proceed,
            ],
        );
    }
}
