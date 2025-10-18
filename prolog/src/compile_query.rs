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
        todo!()
    }
}

