use crate::compiler_bytecode::VarName;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Var { name: VarName },
    Struct { functor: String, arity: usize },
}

impl Cell {
    pub fn new_var(name: VarName) -> Rc<Cell> {
        Rc::new(Cell::Var { name })
    }

    pub fn new_struct(functor: String, arity: usize) -> Rc<Cell> {
        Rc::new(Cell::Struct { functor, arity })
    }

    pub fn is_var(&self) -> bool {
        matches!(self, Cell::Var { .. })
    }
}
