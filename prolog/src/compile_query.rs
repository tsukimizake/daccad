use crate::register_managers::{ArgRegisterManager, XRegisterManager};
use crate::types::{Term, WamInstr, WamReg};

pub struct QueryCompiler {
    arg_register_manager: ArgRegisterManager,
    x_register_manager: XRegisterManager,
}

impl QueryCompiler {
    pub fn new() -> Self {
        QueryCompiler {
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

    pub fn cleanup_regs(&mut self) {
        self.arg_register_manager.reset();
        self.x_register_manager.reset();
    }

    pub fn compile(&mut self, query: Term) -> Vec<WamInstr> {
        match query {
            Term::Atom(name) => vec![WamInstr::PutAtom {
                reg: self.get_next_a(),
                name,
            }],
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
        parse::query,
        types::{WamInstr, WamReg},
    };

    fn test_compile_query(source: &str, expected: Vec<WamInstr>) {
        let mut compiler = QueryCompiler::new();
        let parsed_query = query(source).unwrap().1;
        // For now, we'll compile just the first term in the query
        let instructions = compiler.compile(parsed_query[0].clone());
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
