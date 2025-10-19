use std::collections::HashMap;

use crate::compiler_bytecode::{WamInstr, WamReg};
use crate::parse::Term;
use crate::register_managers::{ArgRegisterManager, XRegisterManager};

#[allow(unused)]
pub(super) struct Compiler {
    declared_vars: HashMap<String, WamReg>, // atomもここ
    arg_register_manager: ArgRegisterManager,
    x_register_manager: XRegisterManager,
}

#[allow(unused)]
impl Compiler {
    pub fn new() -> Self {
        Compiler {
            declared_vars: HashMap::new(),
            arg_register_manager: ArgRegisterManager::new(),
            x_register_manager: XRegisterManager::new(),
        }
    }

    pub fn get_next_a(&mut self) -> WamReg {
        self.arg_register_manager.get_next()
    }

    pub fn get_next_x(&mut self) -> WamReg {
        self.x_register_manager.get_next()
    }

    fn find_var(&self, var: &str) -> Option<&WamReg> {
        if let Some(reg) = self.declared_vars.get(var) {
            return Some(reg);
        }
        None
    }

    fn decl_var(&mut self, var: String, reg: WamReg) {
        self.declared_vars.insert(var, reg);
    }

    pub fn cleanup_regs(&mut self) {
        self.arg_register_manager.reset();
        self.x_register_manager.reset();
    }

    pub fn compile(&mut self, query_terms: Vec<Term>) -> Vec<WamInstr> {
        query_terms
            .into_iter()
            .flat_map(|term| self.compile_query(term))
            .collect()
    }
    fn compile_query(&mut self, term: Term) -> Vec<WamInstr> {
        match term {
            Term::Atom(name) => {
                vec![WamInstr::PutAtom {
                    reg: self.get_next_a(),
                    name,
                }]
            }
            _ => {
                todo!();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compiler_bytecode::{WamInstr, WamReg},
        parse::query,
    };

    fn test_compile_query(source: &str, expected: Vec<WamInstr>) {
        let mut query_compiler = Compiler::new();
        let parsed_query = query(source).unwrap().1;
        // For now, we'll compile just the first term in the query
        let instructions = query_compiler.compile(parsed_query);
        assert_eq!(instructions, expected);
    }

    #[test]
    fn query_atom() {
        test_compile_query(
            "parent.",
            vec![WamInstr::PutAtom {
                name: "parent".to_string(),
                reg: WamReg::A(0),
            }],
        );
    }
}
