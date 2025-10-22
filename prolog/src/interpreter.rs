use std::rc::Rc;

use crate::compiler_bytecode::{WamInstr, WamReg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Ref(Rc<Cell>),
    Struct { functor: String, arity: usize },
    Atom(String),
    Number(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    pub arg_registers: Vec<Cell>,
    pub other_registers: Vec<Cell>,
}

#[allow(unused)]
enum Frame {
    Base {},
    Environment {
        prev_ep: Rc<Frame>,
        return_pc: Rc<WamInstr>,
        registers: Vec<Cell>,
    },
    ChoicePoint {
        // TODO
    },
}

#[allow(unused)]
struct TrailEntry {
    cells_to_revert: Vec<Cell>,
}

fn get_register(registers: &[Cell], index: usize) -> &Cell {
    if index < registers.len() {
        &registers[index]
    } else {
        &Cell::Empty
    }
}

fn set_register(registers: &mut Vec<Cell>, index: usize, value: Cell) {
    if index >= registers.len() {
        registers.resize(index + 1, Cell::Empty);
    }
    registers[index] = value;
}

fn exectute_impl(
    _heap: &mut Vec<Rc<Cell>>,
    _stack: &mut Vec<Rc<Frame>>,
    arg_registers: &mut Vec<Cell>,
    other_registers: &mut Vec<Cell>,
    instructions: &[WamInstr],
    program_counter: &mut usize,
    _env_p: &mut Rc<Frame>,
    _choice_p: &mut Rc<Frame>,
    _trail: &mut Vec<TrailEntry>,
) -> bool {
    if let Some(current_instr) = instructions.get(*program_counter) {
        match current_instr {
            WamInstr::PutAtom { name, reg } => {
                let cell = Cell::Atom(name.clone());
                match reg {
                    WamReg::A(index) => {
                        set_register(arg_registers, *index, cell);
                        true
                    }
                    WamReg::X(index) => {
                        set_register(other_registers, *index, cell);
                        true
                    }
                }
            }

            WamInstr::PutVar { name: _, reg } => match reg {
                WamReg::A(index) => {
                    set_register(arg_registers, *index, Cell::Empty);
                    true
                }
                WamReg::X(index) => {
                    set_register(other_registers, *index, Cell::Empty);
                    true
                }
            },

            WamInstr::GetAtom { name, reg } => {
                let derefed = deref_reg(arg_registers, other_registers, reg);
                match derefed {
                    Cell::Empty => {
                        // レジスタが空の場合、アトムを設定
                        let cell = Cell::Atom(name.clone());
                        match reg {
                            WamReg::A(index) => {
                                set_register(arg_registers, *index, cell);
                                true
                            }
                            WamReg::X(index) => {
                                set_register(other_registers, *index, cell);
                                true
                            }
                        }
                    }
                    Cell::Atom(ref existing_name) if existing_name == name => true,
                    _ => false,
                }
            }
            _ => {
                todo!();
            }
        }
    } else {
        false
    }
}

fn deref_reg(arg_registers: &[Cell], other_registers: &[Cell], wamreg: &WamReg) -> Cell {
    let reg = match wamreg {
        WamReg::A(index) => get_register(arg_registers, *index).clone(),
        WamReg::X(index) => get_register(other_registers, *index).clone(),
    };

    match reg {
        Cell::Ref(rc_cell) => {
            let deref_cell = rc_cell.as_ref();
            deref_cell_recursive(deref_cell)
        }
        _ => reg.clone(),
    }
}

fn deref_cell_recursive(cell: &Cell) -> Cell {
    match cell {
        Cell::Ref(rc_cell) => deref_cell_recursive(&rc_cell),
        _ => cell.clone(),
    }
}

pub fn execute_instructions(instructions: Vec<WamInstr>) -> (Registers, bool) {
    let mut heap = Vec::<Rc<Cell>>::with_capacity(32);
    let stack_head = Rc::new(Frame::Base {});
    let mut stack = vec![stack_head.clone()];
    let mut arg_registers = vec![Cell::Empty; 32];
    let mut other_registers = vec![Cell::Empty; 32];
    let mut program_counter = 0;
    let mut env_p = stack_head.clone();
    let mut choice_p = stack_head;
    let mut trail = Vec::<TrailEntry>::with_capacity(32);

    let success = exectute_impl(
        &mut heap,
        &mut stack,
        &mut arg_registers,
        &mut other_registers,
        &instructions,
        &mut program_counter,
        &mut env_p,
        &mut choice_p,
        &mut trail,
    );

    let res_registers = Registers {
        arg_registers,
        other_registers,
    };
    (res_registers, success)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(db_str: String, query_str: String, expect_regs: Vec<Cell>, expect_res: bool) {
        let db_clauses = crate::parse::database(&db_str).unwrap();
        let (_, query_terms) = crate::parse::query(&query_str).unwrap();
        let db = crate::compile_db::compile_db(db_clauses);
        let query = crate::compile_query::compile_query(query_terms);
        print!("{:?}", query);
        print!("{:?}", db);
        let (regs, result) =
            execute_instructions(query.into_iter().chain(db.into_iter()).collect());
        assert_eq!(result, expect_res);
        assert_eq!(regs.arg_registers, expect_regs);
    }
    fn pad_empties_to_32(regs: Vec<Cell>) -> Vec<Cell> {
        let len = regs.len();
        regs.into_iter()
            .chain(std::iter::repeat(Cell::Empty).take(32 - len))
            .collect()
    }

    #[test]
    fn test_simple_put_atom() {
        test(
            "hello.".to_string(),
            "hello.".to_string(),
            pad_empties_to_32(vec![Cell::Atom("hello".to_string())]),
            true,
        );
    }

    #[allow(unused)]
    //TODO #[test]
    fn test_socrates() {
        test(
            r#"human(socrates). mortal(X) :- human(X)."#.to_string(),
            "mortal(socrates).".to_string(),
            vec![],
            true,
        );
    }
}
