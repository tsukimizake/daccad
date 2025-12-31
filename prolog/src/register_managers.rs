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

    fn test_alloc_registers_helper(source: &str, expected: HashMap<TermId, WamReg>) {
        let parsed_query = query(source).unwrap().1;
        println!("{:?}", parsed_query);
        let term = &parsed_query[0];
        let mut declared_vars = HashMap::new();
        let mut reg_manager = RegisterManager::new();
        let _ = alloc_registers(term, &mut declared_vars, &mut reg_manager);
        assert_eq!(declared_vars, expected);
    }

    #[test]
    fn query_example() {
        test_alloc_registers_helper("p(Z, h(Z,W), f(W)).", {
            let mut map = HashMap::new();
            map.insert(TermId(0), WamReg::X(1));
            map.insert(TermId(1), WamReg::X(3));
            map.insert(TermId(2), WamReg::X(4));
            map.insert(TermId(3), WamReg::X(2));
            map.insert(TermId(4), WamReg::X(6));
            map.insert(TermId(5), WamReg::X(5));
            map.insert(TermId(6), WamReg::X(0));
            map
        });
    }

    // #[test]
    // fn db_example() {
    //     test_alloc_registers_helper("p(f(X), h(Y, f(a)), Y).", {
    //         let mut map = HashMap::new();
    //         map.insert(
    //             RegExpr::Functor {
    //                 name: "p".to_string(),
    //                 arity: 3,
    //                 args: vec![WamReg::X(1), WamReg::X(3), WamReg::X(4)],
    //             },
    //             WamReg::X(0),
    //         );
    //         map.insert(
    //             RegExpr::Functor {
    //                 name: "f".to_string(),
    //                 arity: 1,
    //                 args: vec![WamReg::X(6)],
    //             },
    //             WamReg::X(5),
    //         );
    //         map.insert(RegExpr::Var("X".to_string()), WamReg::X(2));
    //         map.insert(
    //             RegExpr::Functor {
    //                 name: "h".to_string(),
    //                 arity: 2,
    //                 args: vec![WamReg::X(4), WamReg::X(5)],
    //             },
    //             WamReg::X(3),
    //         );
    //         map.insert(RegExpr::Var("Y".to_string()), WamReg::X(4));
    //         map.insert(
    //             RegExpr::Functor {
    //                 name: "f".to_string(),
    //                 arity: 1,
    //                 args: vec![WamReg::X(2)],
    //             },
    //             WamReg::X(1),
    //         );
    //         map.insert(RegExpr::Var("Y".to_string()), WamReg::X(4));
    //         map.insert(
    //             RegExpr::Functor {
    //                 name: "a".to_string(),
    //                 arity: 0,
    //                 args: vec![],
    //             },
    //             WamReg::X(6),
    //         );
    //         map
    //     });
    // }
    // #[test]
    // fn same_var() {
    //     test_alloc_registers_helper("p(X, X).", {
    //         let mut map = HashMap::new();
    //         map.insert(
    //             RegExpr::Functor {
    //                 name: "p".to_string(),
    //                 arity: 2,
    //                 args: vec![WamReg::X(1), WamReg::X(1)],
    //             },
    //             WamReg::X(0),
    //         );
    //         map.insert(RegExpr::Var("X".to_string()), WamReg::X(1));
    //         map
    //     });

    //     test_alloc_registers_helper("p(q(X), Y).", {
    //         let mut map = HashMap::new();
    //         map.insert(
    //             RegExpr::Functor {
    //                 name: "p".to_string(),
    //                 arity: 2,
    //                 args: vec![WamReg::X(1), WamReg::X(3)],
    //             },
    //             WamReg::X(0),
    //         );
    //         map.insert(
    //             RegExpr::Functor {
    //                 name: "q".to_string(),
    //                 arity: 1,
    //                 args: vec![WamReg::X(2)],
    //             },
    //             WamReg::X(1),
    //         );
    //         map.insert(RegExpr::Var("X".to_string()), WamReg::X(2));
    //         map.insert(RegExpr::Var("Y".to_string()), WamReg::X(3));
    //         map
    //     });
    // }
}
