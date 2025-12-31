use crate::compiler_bytecode::WamReg;
use crate::parse::Term;
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum RegExpr {
    Functor {
        name: String,
        arity: usize,
        args: Vec<WamReg>,
    },
    Var(String),
}

pub(crate) fn alloc_registers(
    term: &Term,
    declared_vars: &mut HashMap<RegExpr, WamReg>,
    reg_manager: &mut RegisterManager,
) -> WamReg {
    match term {
        Term::Struct { functor, args } => {
            let reg = reg_manager.get_next();
            let arg_keys = args
                .iter()
                .map(|arg| alloc_registers(arg, declared_vars, reg_manager))
                .collect();
            let f = RegExpr::Functor {
                name: functor.clone(),
                arity: args.len(),
                args: arg_keys,
            };
            declared_vars.insert(f.clone(), reg);
            reg
        }

        Term::Var(name) => {
            let k = RegExpr::Var(name.clone());
            if let Some(&reg) = declared_vars.get(&k) {
                reg
            } else {
                let reg = reg_manager.get_next();
                declared_vars.insert(k.clone(), reg);
                reg
            }
        }
        _ => todo!("{:?}", term),
    }
}

pub(crate) fn to_regkey(term: &Term, reg_map: &HashMap<RegExpr, WamReg>) -> RegExpr {
    match term {
        Term::Struct { functor, args } => RegExpr::Functor {
            name: functor.clone(),
            arity: args.len(),
            args: args
                .iter()
                .map(|arg| to_regkey(arg, reg_map))
                .map(|k| reg_map[&k])
                .collect(),
        },
        Term::Var(name) => RegExpr::Var(name.clone()),
        _ => panic!("Unsupported term for RegKey: {:?}", term),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::query;

    fn test_alloc_registers_helper(source: &str, expected: HashMap<RegExpr, WamReg>) {
        let parsed_query = query(source).unwrap().1;
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
            map.insert(
                RegExpr::Functor {
                    name: "p".to_string(),
                    arity: 3,
                    args: vec![WamReg::X(1), WamReg::X(2), WamReg::X(4)],
                },
                WamReg::X(0),
            );
            map.insert(RegExpr::Var("Z".to_string()), WamReg::X(1));
            map.insert(
                RegExpr::Functor {
                    name: "h".to_string(),
                    arity: 2,
                    args: vec![WamReg::X(1), WamReg::X(3)],
                },
                WamReg::X(2),
            );
            map.insert(RegExpr::Var("W".to_string()), WamReg::X(3));
            map.insert(
                RegExpr::Functor {
                    name: "f".to_string(),
                    arity: 1,
                    args: vec![WamReg::X(3)],
                },
                WamReg::X(4),
            );
            map
        });
    }

    #[test]
    fn db_example() {
        test_alloc_registers_helper("p(f(X), h(Y, f(a)), Y).", {
            let mut map = HashMap::new();
            map.insert(
                RegExpr::Functor {
                    name: "p".to_string(),
                    arity: 3,
                    args: vec![WamReg::X(1), WamReg::X(3), WamReg::X(4)],
                },
                WamReg::X(0),
            );
            map.insert(
                RegExpr::Functor {
                    name: "f".to_string(),
                    arity: 1,
                    args: vec![WamReg::X(6)],
                },
                WamReg::X(5),
            );
            map.insert(RegExpr::Var("X".to_string()), WamReg::X(2));
            map.insert(
                RegExpr::Functor {
                    name: "h".to_string(),
                    arity: 2,
                    args: vec![WamReg::X(4), WamReg::X(5)],
                },
                WamReg::X(3),
            );
            map.insert(RegExpr::Var("Y".to_string()), WamReg::X(4));
            map.insert(
                RegExpr::Functor {
                    name: "f".to_string(),
                    arity: 1,
                    args: vec![WamReg::X(2)],
                },
                WamReg::X(1),
            );
            map.insert(RegExpr::Var("Y".to_string()), WamReg::X(4));
            map.insert(
                RegExpr::Functor {
                    name: "a".to_string(),
                    arity: 0,
                    args: vec![],
                },
                WamReg::X(6),
            );
            map
        });
    }
}
