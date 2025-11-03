use crate::compiler_bytecode::WamReg;
use crate::parse::Term;
use std::collections::HashMap;

#[allow(unused)]
pub(crate) struct RegisterManager {
    count: usize,
}

impl RegisterManager {
    pub fn new() -> Self {
        RegisterManager { count: 0 }
    }

    pub fn get_next(&mut self) -> WamReg {
        let current = self.count;
        self.count += 1;
        WamReg::X(current)
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum RegKey {
    TopFunctor {
        name: String,
        arity: usize,
        args: Vec<WamReg>,
    },
    Functor {
        name: String,
        arity: usize,
        args: Vec<WamReg>,
    },
    Var(String),
}

pub(crate) fn alloc_registers(
    term: &Term,
    declared_vars: &mut HashMap<RegKey, WamReg>,
    reg_manager: &mut RegisterManager,
) -> WamReg {
    match term {
        Term::TopStruct { functor, args } => {
            let reg = reg_manager.get_next();
            let arg_keys = args
                .iter()
                .map(|arg| alloc_registers(arg, declared_vars, reg_manager))
                .collect();
            let f = RegKey::TopFunctor {
                name: functor.clone(),
                arity: args.len(),
                args: arg_keys,
            };
            declared_vars.insert(f.clone(), reg);
            reg
        }

        Term::InnerStruct { functor, args } => {
            let reg = reg_manager.get_next();
            let arg_keys = args
                .iter()
                .map(|arg| alloc_registers(arg, declared_vars, reg_manager))
                .collect();
            let f = RegKey::Functor {
                name: functor.clone(),
                arity: args.len(),
                args: arg_keys,
            };
            declared_vars.insert(f.clone(), reg);
            reg
        }

        Term::InnerAtom(name) => {
            let k = RegKey::Var(name.clone());
            if let Some(&reg) = declared_vars.get(&k) {
                reg
            } else {
                let reg = reg_manager.get_next();
                declared_vars.insert(k.clone(), reg);
                reg
            }
        }
        Term::Var(name) => {
            let k = RegKey::Var(name.clone());
            if let Some(&reg) = declared_vars.get(&k) {
                reg
            } else {
                let reg = reg_manager.get_next();
                declared_vars.insert(k.clone(), reg);
                reg
            }
        }
        Term::TopAtom(name) => {
            let k = RegKey::Var(name.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::query;

    fn test_alloc_registers_helper(source: &str, expected: HashMap<RegKey, WamReg>) {
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
                RegKey::TopFunctor {
                    name: "p".to_string(),
                    arity: 3,
                    args: vec![WamReg::X(1), WamReg::X(2), WamReg::X(4)],
                },
                WamReg::X(0),
            );
            map.insert(RegKey::Var("Z".to_string()), WamReg::X(1));
            map.insert(
                RegKey::Functor {
                    name: "h".to_string(),
                    arity: 2,
                    args: vec![WamReg::X(1), WamReg::X(3)],
                },
                WamReg::X(2),
            );
            map.insert(RegKey::Var("W".to_string()), WamReg::X(3));
            map.insert(
                RegKey::Functor {
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
                RegKey::TopFunctor {
                    name: "p".to_string(),
                    arity: 3,
                    args: vec![WamReg::X(1), WamReg::X(3), WamReg::X(4)],
                },
                WamReg::X(0),
            );
            map.insert(
                RegKey::Functor {
                    name: "f".to_string(),
                    arity: 1,
                    args: vec![WamReg::X(6)],
                },
                WamReg::X(5),
            );
            map.insert(RegKey::Var("X".to_string()), WamReg::X(2));
            map.insert(
                RegKey::Functor {
                    name: "h".to_string(),
                    arity: 2,
                    args: vec![WamReg::X(4), WamReg::X(5)],
                },
                WamReg::X(3),
            );
            map.insert(RegKey::Var("Y".to_string()), WamReg::X(4));
            map.insert(
                RegKey::Functor {
                    name: "f".to_string(),
                    arity: 1,
                    args: vec![WamReg::X(2)],
                },
                WamReg::X(1),
            );
            map.insert(RegKey::Var("Y".to_string()), WamReg::X(4));
            map.insert(RegKey::Var("a".to_string()), WamReg::X(6));
            map
        });
    }
}
