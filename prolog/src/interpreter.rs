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
    arg_registers: Vec<Cell>,
    other_registers: Vec<Cell>,
}

impl Registers {
    fn new() -> Self {
        Self {
            arg_registers: vec![Cell::Empty; 32],
            other_registers: vec![Cell::Empty; 32],
        }
    }

    pub fn get_arg_registers(&self) -> &[Cell] {
        &self.arg_registers
    }
    pub fn get_register<'a>(&'a self, reg: &WamReg) -> &'a Cell {
        let (vec, index) = match reg {
            WamReg::A(index) => (&self.arg_registers, *index),
            WamReg::X(index) => (&self.other_registers, *index),
        };
        if index < vec.len() {
            &vec[index]
        } else {
            &Cell::Empty
        }
    }

    pub fn set_register(&mut self, reg: &WamReg, value: Cell) {
        let (vec, index) = match reg {
            WamReg::A(index) => (&mut self.arg_registers, *index),
            WamReg::X(index) => (&mut self.other_registers, *index),
        };
        if index >= vec.len() {
            vec.resize(index + 1, Cell::Empty);
        }
        vec[index] = value;
    }
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

#[derive(PartialEq, Eq, Debug)]
enum ExecMode {
    Continue,
    ResolvedToTrue,
    ResolvedToFalse,
}

#[derive(PartialEq, Eq, Debug)]
enum ReadWriteMode {
    Read,
    Write,
}

fn exectute_impl(
    heap: &mut Vec<Rc<Cell>>,
    stack: &mut Vec<Rc<Frame>>,
    trail: &mut Vec<TrailEntry>,
    registers: &mut Registers,
    instructions: &[WamInstr],
    program_counter: &mut usize,
    return_address: &mut usize,
    subterm_reg: &mut Rc<Cell>,
    heap_backtrack_reg: &mut Rc<Cell>,
    heap_reg: &mut Rc<Cell>,
    backtrack_cut_reg: &mut Rc<Frame>,
    backtrack_reg: &mut Rc<Frame>,
    _env_reg: &mut Rc<Frame>,
    read_write_mode: &mut ReadWriteMode,
) -> ExecMode {
    if let Some(current_instr) = instructions.get(*program_counter) {
        match current_instr {
            WamInstr::PutAtom { name, reg } => {
                let cell = Cell::Atom(name.clone());
                registers.set_register(reg, cell);
                ExecMode::Continue
            }

            WamInstr::PutStruct {
                functor,
                arity,
                reg,
            } => {
                let obj = Rc::new(Cell::Struct {
                    functor: functor.clone(),
                    arity: *arity,
                });
                heap.push(obj.clone());
                registers.set_register(reg, Cell::Ref(obj));
                ExecMode::Continue
            }

            WamInstr::PutVar { name: _, reg } => {
                registers.set_register(reg, Cell::Empty);
                ExecMode::Continue
            }

            WamInstr::GetAtom { name, reg } => {
                let derefed = deref_reg(registers, reg);
                match derefed {
                    Cell::Empty => {
                        *read_write_mode = ReadWriteMode::Write;
                        let cell = Cell::Atom(name.clone());
                        registers.set_register(reg, cell);
                        ExecMode::Continue
                    }
                    Cell::Atom(existing_name) => {
                        if existing_name == name {
                            *read_write_mode = ReadWriteMode::Read;
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

fn deref_reg<'a>(registers: &'a Registers, wamreg: &WamReg) -> &'a Cell {
    let reg = registers.get_register(wamreg);

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
    let mut registers = Registers::new();
    let mut program_counter = 0;
    let mut env_p = stack_head.clone();
    let mut choice_p = stack_head.clone();
    let mut trail = Vec::<TrailEntry>::with_capacity(32);
    let mut return_address = 0;
    let mut subterm_reg = Rc::new(Cell::Empty);
    let mut heap_backtrack_reg = Rc::new(Cell::Empty);
    let mut heap_reg = Rc::new(Cell::Empty);
    let mut backtrack_cut_reg = stack_head.clone();
    let mut backtrack_reg = stack_head;
    let mut read_write_mode = ReadWriteMode::Read;

    let mut exec_mode = ExecMode::Continue;

    while exec_mode == ExecMode::Continue {
        exec_mode = exectute_impl(
            &mut heap,
            &mut stack,
            &mut trail,
            &mut registers,
            &instructions,
            &mut program_counter,
            &mut return_address,
            &mut subterm_reg,
            &mut heap_backtrack_reg,
            &mut heap_reg,
            &mut backtrack_cut_reg,
            &mut backtrack_reg,
            &mut env_p,
            &mut read_write_mode,
        );
        program_counter += 1;
    }

    (registers, exec_mode == ExecMode::ResolvedToTrue)
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
        assert_eq!(regs.get_arg_registers(), expect_regs);
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
