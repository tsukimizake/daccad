use std::collections::HashMap;
use std::collections::HashSet;

use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::parse::{Term, TermId};

/// コンパイル済みクエリ
#[derive(Debug)]
pub struct CompiledQuery {
    pub instructions: Vec<WamInstr>,
    /// クエリ変数名 → Y レジスタ（実行後の解決に使用）
    pub term_to_reg: HashMap<String, WamReg>,
}

/// クエリ内のすべての変数名を収集する
fn collect_query_vars(terms: &[Term]) -> Vec<String> {
    let mut vars = Vec::new();
    let mut seen = HashSet::new();
    for term in terms {
        collect_vars_recursive(term, &mut vars, &mut seen);
    }
    vars
}

fn collect_vars_recursive(term: &Term, vars: &mut Vec<String>, seen: &mut HashSet<String>) {
    match term {
        Term::Var { name, .. } => {
            if !seen.contains(name) {
                seen.insert(name.clone());
                vars.push(name.clone());
            }
        }
        Term::Struct { args, .. } => {
            for arg in args {
                collect_vars_recursive(arg, vars, seen);
            }
        }
        Term::List { items, tail, .. } => {
            for item in items {
                collect_vars_recursive(item, vars, seen);
            }
            if let Some(t) = tail {
                collect_vars_recursive(t, vars, seen);
            }
        }
        Term::Number { .. } => {}
    }
}

pub fn compile_query(query_terms: Vec<Term>) -> CompiledQuery {
    // クエリ内の変数を収集し、Y レジスタを割り当て
    let query_vars = collect_query_vars(&query_terms);
    let mut term_to_reg: HashMap<String, WamReg> = HashMap::new();
    for (i, var_name) in query_vars.iter().enumerate() {
        term_to_reg.insert(var_name.clone(), WamReg::Y(i));
    }

    // 各ゴールのarityの最大値を求める（other registerの開始位置）
    let max_arity = query_terms
        .iter()
        .map(|t| match t {
            Term::Struct { args, .. } => args.len(),
            _ => 0,
        })
        .max()
        .unwrap_or(0);

    // 変数スコープを共有してコンパイル
    // declared_vars: 変数名 → other register（初回出現時に割り当て）
    let mut declared_vars: HashMap<String, WamReg> = HashMap::new();
    let mut other_reg_counter = max_arity; // other registerのカウンタ

    // クエリ用スタックフレームを確保
    let mut result = Vec::new();
    result.push(WamInstr::Allocate {
        size: query_vars.len(),
    });

    for term in &query_terms {
        result.extend(compile_goal(
            term,
            &mut declared_vars,
            &mut other_reg_counter,
            &term_to_reg,
        ));
    }
    CompiledQuery {
        instructions: result,
        term_to_reg,
    }
}

/// 1つのゴールをコンパイル
fn compile_goal(
    term: &Term,
    declared_vars: &mut HashMap<String, WamReg>,
    other_reg_counter: &mut usize,
    query_var_to_y: &HashMap<String, WamReg>,
) -> Vec<WamInstr> {
    match term {
        Term::Struct { functor, args, .. } => {
            let mut result = Vec::new();

            // 各トップレベル引数を左から右へ処理
            for (i, arg) in args.iter().enumerate() {
                let arg_reg = WamReg::X(i); // 引数レジスタ
                result.extend(compile_top_arg(
                    arg,
                    arg_reg,
                    declared_vars,
                    other_reg_counter,
                    query_var_to_y,
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
/// トップレベル（ファンクタの直接の引数）→ Put
/// 初出 → Var, 2回目以降 → Val
fn compile_top_arg(
    arg: &Term,
    arg_reg: WamReg, // 引数レジスタ（X(0), X(1), ...）
    declared_vars: &mut HashMap<String, WamReg>,
    other_reg_counter: &mut usize,
    query_var_to_y: &HashMap<String, WamReg>,
) -> Vec<WamInstr> {
    match arg {
        Term::Var { name, .. } => {
            if let Some(&existing_reg) = declared_vars.get(name) {
                // 2回目以降 → PutVal
                // arg_reg: 引数位置, with: 初回出現時に割り当てたother register
                vec![WamInstr::PutVal {
                    name: name.clone(),
                    arg_reg,
                    with: existing_reg,
                }]
            } else {
                // 初出 → PutVar
                // with には Y レジスタを使用（クエリ変数を保存するため）
                let y_reg = query_var_to_y
                    .get(name)
                    .copied()
                    .expect("query variable should have Y register");
                declared_vars.insert(name.clone(), y_reg);
                vec![WamInstr::PutVar {
                    name: name.clone(),
                    arg_reg,
                    with: y_reg,
                }]
            }
        }
        Term::Struct { .. } => {
            let mut struct_regs = HashMap::new();
            compile_struct_arg(
                arg,
                arg_reg,
                declared_vars,
                &mut struct_regs,
                other_reg_counter,
                query_var_to_y,
            )
        }
        _ => todo!("{:?}", arg),
    }
}

/// 構造体引数をコンパイル（内部構造体を先に処理）
/// ネストした位置（複合項の中）→ Set
/// 初出 → Var, 2回目以降 → Val
fn compile_struct_arg(
    arg: &Term,
    arg_reg: WamReg, // 引数レジスタ（X(0), X(1), ...）
    declared_vars: &mut HashMap<String, WamReg>,
    // ネストした構造体のレジスタを管理するマップ
    struct_regs: &mut HashMap<TermId, WamReg>,
    other_reg_counter: &mut usize,
    query_var_to_y: &HashMap<String, WamReg>,
) -> Vec<WamInstr> {
    match arg {
        Term::Struct { functor, args, .. } => {
            let mut result = Vec::new();

            // まず内部の構造体を再帰的にコンパイル
            for nested_arg in args {
                if matches!(nested_arg, Term::Struct { .. }) {
                    // 内部構造体には新しいother registerを割り当て
                    let nested_reg = WamReg::X(*other_reg_counter);
                    *other_reg_counter += 1;
                    struct_regs.insert(nested_arg.id(), nested_reg);
                    result.extend(compile_struct_arg(
                        nested_arg,
                        nested_reg,
                        declared_vars,
                        struct_regs,
                        other_reg_counter,
                        query_var_to_y,
                    ));
                }
            }

            // PutStruct を発行（arg_regは引数レジスタ）
            result.push(WamInstr::PutStruct {
                functor: functor.clone(),
                arity: args.len(),
                arg_reg,
            });

            // 各引数について SetVar/SetVal を発行
            for nested_arg in args {
                match nested_arg {
                    Term::Var { name, .. } => {
                        if let Some(&existing_reg) = declared_vars.get(name) {
                            // 2回目以降 → SetVal（declared_varsに登録されたレジスタを使用）
                            result.push(WamInstr::SetVal {
                                name: name.clone(),
                                reg: existing_reg,
                            });
                        } else {
                            // 初出 → SetVar（Y レジスタを使用）
                            let y_reg = query_var_to_y
                                .get(name)
                                .copied()
                                .expect("query variable should have Y register");
                            declared_vars.insert(name.clone(), y_reg);
                            result.push(WamInstr::SetVar {
                                name: name.clone(),
                                reg: y_reg,
                            });
                        }
                    }
                    Term::Struct { .. } => {
                        // 既にコンパイル済みなので SetVal（struct_regsから取得）
                        result.push(WamInstr::SetVal {
                            name: nested_arg.get_name().to_string(),
                            reg: struct_regs[&nested_arg.id()],
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
        let compiled = compile_query(parsed_query);
        assert!(
            compiled.instructions == expected,
            "Mismatch for query: {}\n\nActual:\n{:?}\nExpected:\n{:?}",
            source,
            WamInstrs(&compiled.instructions),
            WamInstrs(&expected)
        );
    }

    #[test]
    fn top_atom() {
        // arity=0の場合はAllocate(0) + CallTempのみ
        test_compile_query(
            "parent.",
            vec![
                WamInstr::Allocate { size: 0 },
                WamInstr::CallTemp {
                    predicate: "parent".to_string(),
                    arity: 0,
                },
            ],
        );
    }

    #[test]
    fn book_example() {
        // p(Z, h(Z,W), f(W))
        // クエリ変数: Z=Y(0), W=Y(1)
        test_compile_query(
            "p(Z, h(Z,W), f(W)).",
            vec![
                WamInstr::Allocate { size: 2 }, // Z, W の2変数
                WamInstr::PutVar {
                    name: "Z".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::PutStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    arg_reg: WamReg::X(1),
                },
                WamInstr::SetVal {
                    name: "Z".to_string(),
                    reg: WamReg::Y(0),
                },
                WamInstr::SetVar {
                    name: "W".to_string(),
                    reg: WamReg::Y(1),
                },
                WamInstr::PutStruct {
                    functor: "f".to_string(),
                    arity: 1,
                    arg_reg: WamReg::X(2),
                },
                WamInstr::SetVal {
                    name: "W".to_string(),
                    reg: WamReg::Y(1),
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
        // クエリ変数: X=Y(0), Y=Y(1)
        test_compile_query(
            "p(a(X), a(Y)).",
            vec![
                WamInstr::Allocate { size: 2 }, // X, Y の2変数
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    arg_reg: WamReg::X(0),
                },
                WamInstr::SetVar {
                    name: "X".to_string(),
                    reg: WamReg::Y(0),
                },
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    arg_reg: WamReg::X(1),
                },
                WamInstr::SetVar {
                    name: "Y".to_string(),
                    reg: WamReg::Y(1),
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
        // クエリ変数: X=Y(0), Y=Y(1)
        test_compile_query(
            "p(X), q(Y).",
            vec![
                WamInstr::Allocate { size: 2 }, // X, Y の2変数
                WamInstr::PutVar {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::Y(0),
                },
                WamInstr::CallTemp {
                    predicate: "p".to_string(),
                    arity: 1,
                },
                WamInstr::PutVar {
                    name: "Y".to_string(),
                    arg_reg: WamReg::X(0), // q(Y)の第0引数
                    with: WamReg::Y(1),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 1,
                },
            ],
        );
    }
}