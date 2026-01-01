use crate::compiler_bytecode::WamReg;
use crate::parse::{Term, TermId};
use std::collections::HashMap;

// コンパイラ用のレジスタ割り当てくん
#[allow(unused)]
pub(crate) struct RegisterManager {
    count: usize,
}

impl RegisterManager {
    pub fn new() -> Self {
        RegisterManager { count: 0 }
    }

    fn get_next(&mut self) -> WamReg {
        let current = self.count;
        self.count += 1;
        WamReg::X(current)
    }
}

pub(crate) fn alloc_registers(
    term: &Term,
    declared_vars: &mut HashMap<TermId, WamReg>,
    reg_manager: &mut RegisterManager,
) -> WamReg {
    match term {
        Term::Struct {
            args, id: term_id, ..
        } => {
            let reg = reg_manager.get_next();
            // TODO 幅優先探索にしてトップ引数を先に割り当てる
            args.iter().for_each(|arg| {
                alloc_registers(arg, declared_vars, reg_manager);
            });
            declared_vars.insert(*term_id, reg);
            reg
        }

        Term::Var { id: term_id, .. } => {
            let reg = reg_manager.get_next();
            declared_vars.insert(*term_id, reg);
            reg
        }
        _ => todo!("{:?}", term),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::query;

    #[derive(Debug)]
    struct ExpectedTerm {
        reg: WamReg,
        kind: ExpectedKind,
    }

    #[derive(Debug)]
    enum ExpectedKind {
        Var(String),
        Struct {
            functor: String,
            args: Vec<ExpectedTerm>,
        },
    }

    fn var(name: &str, reg: WamReg) -> ExpectedTerm {
        ExpectedTerm {
            reg,
            kind: ExpectedKind::Var(name.to_string()),
        }
    }

    fn structure(functor: &str, reg: WamReg, args: Vec<ExpectedTerm>) -> ExpectedTerm {
        ExpectedTerm {
            reg,
            kind: ExpectedKind::Struct {
                functor: functor.to_string(),
                args,
            },
        }
    }

    fn build_expected_map(term: &Term, expected: &ExpectedTerm, out: &mut HashMap<TermId, WamReg>) {
        match (term, &expected.kind) {
            (Term::Var { name, .. }, ExpectedKind::Var(expected_name)) => {
                assert_eq!(name, expected_name);
                out.insert(term.id(), expected.reg);
            }
            (
                Term::Struct { functor, args, .. },
                ExpectedKind::Struct {
                    functor: expected_functor,
                    args: expected_args,
                },
            ) => {
                assert_eq!(functor, expected_functor);
                assert_eq!(args.len(), expected_args.len());
                out.insert(term.id(), expected.reg);
                for (arg, expected_arg) in args.iter().zip(expected_args) {
                    build_expected_map(arg, expected_arg, out);
                }
            }
            _ => panic!("expected {:?}, got {:?}", expected, term),
        }
    }

    fn expected_map(term: &Term, expected: &ExpectedTerm) -> HashMap<TermId, WamReg> {
        let mut map = HashMap::new();
        build_expected_map(term, expected, &mut map);
        map
    }

    fn test_alloc_registers(source: &str, expected: ExpectedTerm) {
        let parsed_query = query(source).unwrap().1;
        println!("{:?}", parsed_query);
        let term = &parsed_query[0];
        let mut declared_vars = HashMap::new();
        let mut reg_manager = RegisterManager::new();
        let _ = alloc_registers(term, &mut declared_vars, &mut reg_manager);
        let expected = expected_map(term, &expected);
        assert_eq!(declared_vars, expected);
    }

    #[test]
    fn query_example() {
        test_alloc_registers(
            "p(Z, h(Z,W), f(W)).",
            structure(
                "p",
                WamReg::X(0),
                vec![
                    var("Z", WamReg::X(1)),
                    structure(
                        "h",
                        WamReg::X(2),
                        vec![var("Z", WamReg::X(3)), var("W", WamReg::X(4))],
                    ),
                    structure("f", WamReg::X(5), vec![var("W", WamReg::X(6))]),
                ],
            ),
        );
    }

    #[test]
    fn db_example() {
        test_alloc_registers(
            "p(f(X), h(Y, f(a)), Y).",
            structure(
                "p",
                WamReg::X(0),
                vec![
                    structure("f", WamReg::X(1), vec![var("X", WamReg::X(2))]),
                    structure(
                        "h",
                        WamReg::X(3),
                        vec![
                            var("Y", WamReg::X(4)),
                            structure(
                                "f",
                                WamReg::X(5),
                                vec![structure("a", WamReg::X(6), vec![])],
                            ),
                        ],
                    ),
                    var("Y", WamReg::X(7)),
                ],
            ),
        );
    }
    #[test]
    fn same_var() {
        test_alloc_registers(
            "p(X, X).",
            structure(
                "p",
                WamReg::X(0),
                vec![var("X", WamReg::X(1)), var("X", WamReg::X(2))],
            ),
        );

        test_alloc_registers(
            "p(q(X), Y).",
            structure(
                "p",
                WamReg::X(0),
                vec![
                    structure("q", WamReg::X(1), vec![var("X", WamReg::X(2))]),
                    var("Y", WamReg::X(3)),
                ],
            ),
        );
    }
}
