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

#[derive(PartialEq, Eq, Debug)]
enum ExecMode {
    Continue,
    ResolvedToTrue,
    ResolvedToFalse,
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
) -> ExecMode {
    if let Some(current_instr) = instructions.get(*program_counter) {
        match current_instr {
            WamInstr::PutAtom { name, reg } => {
                let cell = Cell::Atom(name.clone());
                match reg {
                    WamReg::A(index) => {
                        set_register(arg_registers, *index, cell);
                        ExecMode::Continue
                    }
                    WamReg::X(index) => {
                        set_register(other_registers, *index, cell);
                        ExecMode::Continue
                    }
                }
            }

            WamInstr::PutVar { name: _, reg } => match reg {
                WamReg::A(index) => {
                    set_register(arg_registers, *index, Cell::Empty);
                    ExecMode::Continue
                }
                WamReg::X(index) => {
                    set_register(other_registers, *index, Cell::Empty);
                    ExecMode::Continue
                }
            },

            WamInstr::GetAtom { name, reg } => {
                let derefed = deref_reg(arg_registers, other_registers, reg);
                match derefed {
                    Cell::Empty => {
                        let cell = Cell::Atom(name.clone());
                        match reg {
                            WamReg::A(index) => {
                                set_register(arg_registers, *index, cell);
                                ExecMode::Continue
                            }
                            WamReg::X(index) => {
                                set_register(other_registers, *index, cell);
                                ExecMode::Continue
                            }
                        }
                    }
                    Cell::Atom(existing_name) => {
                        if existing_name == name {
                            ExecMode::Continue
                        } else {
                            ExecMode::ResolvedToFalse
                        }
                    }
                    _ => todo!(),
                }
            }
            WamInstr::Call {
                predicate: _,
                arity: _,
                to_linum,
            } => {
                *program_counter = *to_linum;
                ExecMode::Continue
            }
            WamInstr::Label { name: _, arity: _ } => ExecMode::Continue,
            WamInstr::Proceed => {
                ExecMode::ResolvedToTrue // TODO stackをたどるか何かして解決すべきpredicateが残っていないかcheck
            }
            WamInstr::Error { message } => {
                println!("{}", message);
                ExecMode::ResolvedToFalse
            }

            _ => {
                todo!();
            }
        }
    } else {
        todo!();
    }
}

fn deref_reg<'a>(
    arg_registers: &'a [Cell],
    other_registers: &'a [Cell],
    wamreg: &WamReg,
) -> &'a Cell {
    let reg = match wamreg {
        WamReg::A(index) => get_register(arg_registers, *index),
        WamReg::X(index) => get_register(other_registers, *index),
    };

    match reg {
        Cell::Ref(rc_cell) => {
            let deref_cell = rc_cell.as_ref();
            deref_cell_recursive(deref_cell)
        }
        _ => reg,
    }
}

fn deref_cell_recursive<'a>(cell: &'a Cell) -> &'a Cell {
    match cell {
        Cell::Ref(rc_cell) => deref_cell_recursive(&rc_cell),
        _ => cell,
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

    let mut exec_mode = ExecMode::Continue;

    while exec_mode == ExecMode::Continue {
        exec_mode = exectute_impl(
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
        program_counter += 1;
    }

    let res_registers = Registers {
        arg_registers,
        other_registers,
    };
    (res_registers, exec_mode == ExecMode::ResolvedToTrue)
}

#[cfg(test)]
mod tests {
    use crate::compile_link;

    use super::*;

    fn test(db_str: String, query_str: String, expect_regs: Vec<Cell>, expect_res: bool) {
        let db_clauses = crate::parse::database(&db_str).unwrap();
        let (_, query_terms) = crate::parse::query(&query_str).unwrap();
        let db = crate::compile_db::compile_db(db_clauses);
        let query = crate::compile_query::compile_query(query_terms);
        let all_instructions = compile_link::compile_link(query, db);
        let (regs, result) = execute_instructions(all_instructions);
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
            pad_empties_to_32(vec![]), // TopAtomは引数レジスタに値を設定しない
            true,
        );
        test(
            "hello.".to_string(),
            "bye.".to_string(),
            pad_empties_to_32(vec![]), // TopAtomは引数レジスタに値を設定しない
            false,
        );
    }

    #[test]
    fn test_socrates_who() {
        test(
            r#"mortal(socrates)."#.to_string(),
            "mortal(X).".to_string(),
            pad_empties_to_32(vec![Cell::Atom("socrates".into())]),
            true,
        );
    }

    #[test]
    fn test_socrates_immortal() {
        test(
            r#"mortal(socrates)."#.to_string(),
            "mortal(dracle).".to_string(),
            pad_empties_to_32(vec![Cell::Atom("dracle".into())]),
            false,
        );
    }
    //#[test]
    #[allow(unused)]
    fn test_socrates_all_mortal() {
        test(
            r#"mortal(X)."#.to_string(),
            "mortal(socrates).".to_string(),
            vec![],
            true,
        );
    }
}
