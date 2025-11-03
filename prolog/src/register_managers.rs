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
