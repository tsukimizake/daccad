use std::collections::HashMap;

use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::parse::{Term, TermId};
use crate::register_managers::{RegisterManager, alloc_registers};

/// コンパイル済みクエリ
#[derive(Debug)]
pub struct CompiledQuery {
    pub instructions: Vec<WamInstr>,
    pub term_to_reg: HashMap<TermId, WamReg>,
}

pub fn compile_query(query_terms: Vec<Term>) -> CompiledQuery {
    // すべてのゴールに対して累積的にレジスタを割り当て
    let mut reg_map = HashMap::new();
    let mut reg_manager = RegisterManager::new();
    for term in &query_terms {
        alloc_registers(term, &mut reg_map, &mut reg_manager);
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
    let mut result = Vec::new();
    for term in &query_terms {
        result.extend(compile_goal(
            term,
            &mut declared_vars,
            &mut other_reg_counter,
        ));
    }
    CompiledQuery {
        instructions: result,
        term_to_reg: reg_map,
    }
}

/// 1つのゴールをコンパイル
fn compile_goal(
    term: &Term,
    declared_vars: &mut HashMap<String, WamReg>,
    other_reg_counter: &mut usize,
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
                // arg_reg: 引数位置, with: 新しいother register
                let with = WamReg::X(*other_reg_counter);
                *other_reg_counter += 1;
                declared_vars.insert(name.clone(), with);
                vec![WamInstr::PutVar {
                    name: name.clone(),
                    arg_reg,
                    with,
                }]
            }
        }
        Term::Struct { .. } => {
            let mut struct_regs = HashMap::new();
            compile_struct_arg(arg, arg_reg, declared_vars, &mut struct_regs, other_reg_counter)
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
                            // 初出 → SetVar（新しいother registerを割り当て）
                            let reg = WamReg::X(*other_reg_counter);
                            *other_reg_counter += 1;
                            declared_vars.insert(name.clone(), reg);
                            result.push(WamInstr::SetVar {
                                name: name.clone(),
                                reg,
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
                    arg_reg: WamReg::X(0),
                    with: WamReg::X(3),
                },
                WamInstr::PutStruct {
                    functor: "h".to_string(),
                    arity: 2,
                    arg_reg: WamReg::X(1),
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
                    arg_reg: WamReg::X(2),
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
                    arg_reg: WamReg::X(0),
                },
                WamInstr::SetVar {
                    name: "X".to_string(),
                    reg: WamReg::X(2),
                },
                WamInstr::PutStruct {
                    functor: "a".to_string(),
                    arity: 1,
                    arg_reg: WamReg::X(1),
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
        // max_arity = 1, other registerはX(1)から開始
        // X: 第0引数, with=X(1)
        // Y: 第0引数（q内）, with=X(2)
        test_compile_query(
            "p(X), q(Y).",
            vec![
                WamInstr::PutVar {
                    name: "X".to_string(),
                    arg_reg: WamReg::X(0),
                    with: WamReg::X(1),
                },
                WamInstr::CallTemp {
                    predicate: "p".to_string(),
                    arity: 1,
                },
                WamInstr::PutVar {
                    name: "Y".to_string(),
                    arg_reg: WamReg::X(0), // q(Y)の第0引数
                    with: WamReg::X(2),
                },
                WamInstr::CallTemp {
                    predicate: "q".to_string(),
                    arity: 1,
                },
            ],
        );
    }
}
