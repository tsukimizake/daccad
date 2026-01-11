use std::collections::HashMap;

use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::parse::{Term, TermId};
use crate::register_managers::{RegisterManager, alloc_registers};

pub fn compile_query(query_terms: Vec<Term>) -> Vec<WamInstr> {
    // すべてのゴールに対して累積的にレジスタを割り当て
    let mut reg_map = HashMap::new();
    let mut reg_manager = RegisterManager::new();
    for term in &query_terms {
        alloc_registers(term, &mut reg_map, &mut reg_manager);
    }

    // すべてのゴールから変数の出現を収集
    let mut var_occurrences: HashMap<String, Vec<WamReg>> = HashMap::new();
    for term in &query_terms {
        collect_vars_recursive(term, &reg_map, &mut var_occurrences);
    }

    // 変数スコープを共有してコンパイル
    let mut declared_vars: HashMap<String, WamReg> = HashMap::new();
    let mut result = Vec::new();
    for term in &query_terms {
        result.extend(compile_goal(
            term,
            &reg_map,
            &var_occurrences,
            &mut declared_vars,
        ));
    }
    result
}

fn collect_vars_recursive(
    term: &Term,
    reg_map: &HashMap<TermId, WamReg>,
    result: &mut HashMap<String, Vec<WamReg>>,
) {
    match term {
        Term::Var { id, name, .. } => {
            result.entry(name.clone()).or_default().push(reg_map[id]);
        }
        Term::Struct { args, .. } => {
            for arg in args {
                collect_vars_recursive(arg, reg_map, result);
            }
        }
        _ => {}
    }
}

/// 1つのゴールをコンパイル
fn compile_goal(
    term: &Term,
    reg_map: &HashMap<TermId, WamReg>,
    var_occurrences: &HashMap<String, Vec<WamReg>>,
    declared_vars: &mut HashMap<String, WamReg>,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args, .. } => {
            let mut result = Vec::new();

            // 各トップレベル引数を左から右へ処理
            for arg in args {
                result.extend(compile_top_arg(
                    arg,
                    reg_map,
                    var_occurrences,
                    declared_vars,
                ));
            }

            // CallTemp を発行
            result.push(WamInstr::CallTemp {
                predicate: functor.clone(),
                arity: args.len(),
            });

            result
        }
        _ => todo!("{:?}", term),
    }
}

/// トップレベル引数をコンパイル
fn compile_top_arg(
    arg: &Term,
    reg_map: &HashMap<TermId, WamReg>,
    var_occurrences: &HashMap<String, Vec<WamReg>>,
    declared_vars: &mut HashMap<String, WamReg>,
) -> Vec<WamInstr> {
    match arg {
        Term::Var { name, .. } => {
            let reg = reg_map[&arg.id()];

            if let Some(&existing_reg) = declared_vars.get(name) {
                // 2回目以降
                vec![WamInstr::SetVal {
                    name: name.clone(),
                    reg: existing_reg,
                }]
            } else {
                // 初回出現
                let occurrences = &var_occurrences[name];
                if occurrences.len() > 1 {
                    // 他にも出現がある → PutVar
                    let reg2 = occurrences[1]; // 2番目の出現のレジスタ
                    declared_vars.insert(name.clone(), reg2);
                    vec![WamInstr::PutVar {
                        name: name.clone(),
                        argreg: reg,
                        reg2,
                    }]
                } else {
                    // 1回しか出現しない → SetVar
                    declared_vars.insert(name.clone(), reg);
                    vec![WamInstr::SetVar {
                        name: name.clone(),
                        reg,
                    }]
                }
            }
        }
        Term::Struct { .. } => compile_struct_arg(arg, reg_map, var_occurrences, declared_vars),
        _ => todo!("{:?}", arg),
    }
}

/// 構造体引数をコンパイル（内部構造体を先に処理）
fn compile_struct_arg(
    arg: &Term,
    reg_map: &HashMap<TermId, WamReg>,
    var_occurrences: &HashMap<String, Vec<WamReg>>,
    declared_vars: &mut HashMap<String, WamReg>,
) -> Vec<WamInstr> {
    match arg {
        Term::Struct { functor, args, .. } => {
            let mut result = Vec::new();

            // まず内部の構造体を再帰的にコンパイル
            for nested_arg in args {
                if matches!(nested_arg, Term::Struct { .. }) {
                    result.extend(compile_struct_arg(
                        nested_arg,
                        reg_map,
                        var_occurrences,
                        declared_vars,
                    ));
                }
            }

            // PutStruct を発行
            result.push(WamInstr::PutStruct {
                functor: functor.clone(),
                arity: args.len(),
                reg: reg_map[&arg.id()],
            });

            // 各引数について SetVar/SetVal を発行
            for nested_arg in args {
                match nested_arg {
                    Term::Var { name, .. } => {
                        if let Some(&existing_reg) = declared_vars.get(name) {
                            // 2回目以降
                            result.push(WamInstr::SetVal {
                                name: name.clone(),
                                reg: existing_reg,
                            });
                        } else {
                            // 初回
                            let reg = reg_map[&nested_arg.id()];
                            declared_vars.insert(name.clone(), reg);
                            result.push(WamInstr::SetVar {
                                name: name.clone(),
                                reg,
                            });
                        }
                    }
                    Term::Struct { .. } => {
                        // 既にコンパイル済みなので SetVal
                        result.push(WamInstr::SetVal {
                            name: nested_arg.get_name().to_string(),
                            reg: reg_map[&nested_arg.id()],
                        });
                    }
                    _ => todo!("{:?}", nested_arg),
                }
            }

            result
        }
        _ => todo!("{:?}", arg),
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
        // arity=0の場合はCallTempのみ
        test_compile_query(
            "parent.",
            vec![WamInstr::CallTemp {
                predicate: "parent".to_string(),
                arity: 0,
            }],
        );
    }

    #[test]
    fn book_example() {
        // p(Z, h(Z,W), f(W))
        // トップレベルpにはレジスタなし
        // 引数: Z=X(0), h=X(1), f=X(2), Z(in h)=X(3), W(in h)=X(4), W(in f)=X(5)
        test_compile_query(
            "p(Z, h(Z,W), f(W)).",
            vec![
                WamInstr::PutVar {
                    name: "Z".to_string(),
                    argreg: WamReg::X(0),
                    reg2: WamReg::X(3),
                },
                WamInstr::PutStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    reg: WamReg::X(1),
                },
                WamInstr::SetVal {
                    name: "Z".to_string(),
                    reg: WamReg::X(3),
                },
                WamInstr::SetVar {
                    name: "W".to_string(),
                    reg: WamReg::X(4),
                },
                WamInstr::PutStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    reg: WamReg::X(2),
                },
                WamInstr::SetVal {
                    name: "W".to_string(),
                    reg: WamReg::X(4),
                },
                WamInstr::CallTemp {
                    predicate: "p".to_string(),
                    arity: 3,
                },
            ],
        )
    }

    #[test]
    fn same_functor_other_arg() {
        // p(a(X), a(Y))
        // 引数: a=X(0), a=X(1), X=X(2), Y=X(3)
        test_compile_query(
            "p(a(X), a(Y)).",
            vec![
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    reg: WamReg::X(0),
                },
                WamInstr::SetVar {
                    name: "X".to_string(),
                    reg: WamReg::X(2),
                },
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    reg: WamReg::X(1),
                },
                WamInstr::SetVar {
                    name: "Y".to_string(),
                    reg: WamReg::X(3),
                },
                WamInstr::CallTemp {
                    predicate: "p".to_string(),
                    arity: 2,
                },
            ],
        );
    }

    #[test]
    fn two_goals() {
        // p(X), q(Y) - カンマ区切りの複数ゴール
        // 引数: X=X(0), Y=X(1)
        test_compile_query(
            "p(X), q(Y).",
            vec![
                WamInstr::SetVar {
                    name: "X".to_string(),
                    reg: WamReg::X(0),
                },
                WamInstr::CallTemp {
                    predicate: "p".to_string(),
                    arity: 1,
                },
                WamInstr::SetVar {
                    name: "Y".to_string(),
                    reg: WamReg::X(1),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 1,
                },
            ],
        );
    }
}
