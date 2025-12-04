use std::{cell::RefCell, rc::Rc};

use crate::compiler_bytecode::{WamInstr, WamReg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Var {
    Unbound,
    Bound(Rc<Cell>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Var(RefCell<Var>),
    Ref(Rc<Cell>),
    Struct {
        functor: String,
        arity: usize,
        children: Vec<Rc<Cell>>,
    },
    Number(i64),
}

impl Cell {
    pub fn new_var() -> Self {
        Cell::Var(RefCell::new(Var::Unbound))
    }

    pub fn unsafe_replace_var(&self, new_value: Cell) {
        if let Cell::Var(var_cell) = self {
            *var_cell.borrow_mut() = Var::Bound(Rc::new(new_value));
        } else {
            panic!("replace_var called on non-var cell");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    registers: Vec<Cell>,
}

impl Registers {
    fn new() -> Self {
        Self {
            registers: vec![Cell::new_var(); 32],
        }
    }

    pub fn get_arg_registers(&self) -> &[Cell] {
        &self.registers
    }
    pub fn get_register<'a>(&'a mut self, reg: &WamReg) -> &'a Cell {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        if index < self.registers.len() {
            &self.registers[index]
        } else {
            let r = Cell::new_var();
            self.registers.push(r);
            &self.registers[index]
        }
    }

    pub fn set_register(&mut self, reg: &WamReg, value: Cell) {
        let index = match reg {
            WamReg::X(index) => *index,
        };
        if index >= self.registers.len() {
            self.registers.resize(index + 1, Cell::new_var());
        }
        self.registers[index] = value;
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

fn exectute_impl(
    heap: &mut Vec<Rc<Cell>>,
    _stack: &mut Vec<Rc<Frame>>,
    _trail: &mut Vec<TrailEntry>,
    registers: &mut Registers,
    instructions: &[WamInstr],
    program_counter: &mut usize,
    _return_address: &mut usize,
    _subterm_reg: &mut Rc<Cell>,
    _heap_backtrack_reg: &mut Rc<Cell>,
    _heap_reg: &mut Rc<Cell>,
    _backtrack_cut_reg: &mut Rc<Frame>,
    _backtrack_reg: &mut Rc<Frame>,
    _env_reg: &mut Rc<Frame>,
    exec_mode: &mut ExecMode,
) {
    if let Some(current_instr) = instructions.get(*program_counter) {
        match current_instr {
            WamInstr::PutStruct {
                functor,
                arity,
                reg,
            } => {
                // TODO arity0なら即座に構造体を作成
                let ob = Rc::new(Cell::new_var());
                let mut subterms = Vec::new();
                heap.push(ob.clone());
                registers.set_register(reg, Cell::Ref(ob.clone()));
                eval_put_struct_children(
                    instructions,
                    program_counter,
                    registers,
                    heap,
                    *arity,
                    &mut subterms,
                );
                let structure = Rc::new(Cell::Struct {
                    functor: functor.clone(),
                    arity: *arity,
                    children: subterms,
                });
                ob.unsafe_replace_var((*structure).clone());
            }

            WamInstr::SetVar { reg, name: _ } => {
                let ob = Rc::new(Cell::new_var());
                heap.push(ob.clone());
                registers.set_register(reg, Cell::Ref(ob));
            }
            WamInstr::SetVal { reg, name: _ } => {
                let value = registers.get_register(reg).clone();
                heap.push(Rc::new(value));
            }

            WamInstr::PutVar { name: _, reg } => {
                registers.set_register(reg, Cell::new_var());
            }

            WamInstr::GetStruct {
                functor,
                arity,
                reg,
            } => {
                let derefed = deref_reg(registers, reg);
                match derefed {
                    Cell::Var(v) => {
                        let is_unbound = matches!(*v.borrow(), Var::Unbound);
                        if is_unbound {
                            // write mode
                            let cell = Cell::Ref(Rc::new(Cell::Struct {
                                functor: functor.clone(),
                                arity: *arity,
                                children: Vec::with_capacity(*arity),
                            }));
                            derefed.unsafe_replace_var(cell.clone());
                            registers.set_register(reg, cell.clone());
                        } else {
                            if let Var::Bound(bound) = &*v.borrow() {
                                if let Cell::Struct {
                                    functor: existing_functor,
                                    arity: existing_arity,
                                    children: _,
                                } = bound.as_ref()
                                {
                                    // read mode
                                    if *existing_functor != *functor || *existing_arity != *arity {
                                        *exec_mode = ExecMode::ResolvedToFalse;
                                    }
                                }
                            }
                        }
                    }
                    Cell::Struct {
                        functor: existing_functor,
                        arity: existing_arity,
                        children: _,
                    } => {
                        // read mode
                        // TODO unify children
                        if existing_functor != functor || existing_arity != arity {
                            *exec_mode = ExecMode::ResolvedToFalse;
                        }
                    }
                    _ => {
                        *exec_mode = ExecMode::ResolvedToFalse;
                    }
                }
            }
            WamInstr::Call {
                predicate: _,
                arity: _,
                to_linum,
            } => {
                *program_counter = *to_linum;
            }
            WamInstr::Label { name: _, arity: _ } => {}
            WamInstr::Proceed => {
                *exec_mode = ExecMode::ResolvedToTrue; // TODO stackをたどるか何かして解決すべきpredicateが残っていないかcheck
            }
            WamInstr::Error { message } => {
                println!("{}", message);
                *exec_mode = ExecMode::ResolvedToFalse;
            }

            instr => {
                todo!("{:?}", instr);
            }
        }
    } else {
        todo!("current_instr is None");
    }
}

fn eval_put_struct_children(
    instructions: &[WamInstr],
    program_counter: &mut usize,
    registers: &mut Registers,
    heap: &mut Vec<Rc<Cell>>,
    arity: usize,
    subterms: &mut Vec<Rc<Cell>>,
) {
    for _ in 0..arity {
        *program_counter += 1;
        let current_instr = instructions.get(*program_counter).unwrap();
        match current_instr {
            WamInstr::SetVar { reg, name: _ } => {
                let ob = Rc::new(Cell::new_var());
                heap.push(ob.clone());
                registers.set_register(reg, Cell::Ref(ob.clone()));
                subterms.push(ob);
            }
            WamInstr::SetVal { reg, name: _ } => {
                let value = Rc::new(registers.get_register(reg).clone());
                heap.push(value.clone());
                subterms.push(value);
            }
            _ => {
                panic!("Expected SetVar or SetVal in struct children");
            }
        }
    }
}

fn deref_reg<'a>(registers: &'a mut Registers, wamreg: &WamReg) -> &'a Cell {
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
    // let mut choice_p = stack_head.clone();
    let mut trail = Vec::<TrailEntry>::with_capacity(32);
    let mut return_address = 0;
    let mut subterm_reg = Rc::new(Cell::new_var());
    let mut heap_backtrack_reg = Rc::new(Cell::new_var());
    let mut heap_reg = Rc::new(Cell::new_var());
    let mut backtrack_cut_reg = stack_head.clone();
    let mut backtrack_reg = stack_head;

    let mut exec_mode = ExecMode::Continue;

    while exec_mode == ExecMode::Continue {
        exectute_impl(
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
            &mut exec_mode,
        );
        program_counter += 1;
    }

    (registers, exec_mode == ExecMode::ResolvedToTrue)
}

#[cfg(test)]
mod tests {
    use crate::compile_link;

    use super::*;

    fn normalize_cell(cell: &Cell) -> Cell {
        match cell {
            Cell::Var(v) => match &*v.borrow() {
                Var::Unbound => Cell::new_var(),
                Var::Bound(bound) => normalize_cell(bound),
            },
            Cell::Ref(rc_cell) => normalize_cell(rc_cell),
            Cell::Struct {
                functor,
                arity,
                children,
            } => Cell::Struct {
                functor: functor.clone(),
                arity: *arity,
                children: children
                    .iter()
                    .map(|c| Rc::new(normalize_cell(c)))
                    .collect(),
            },
            Cell::Number(n) => Cell::Number(*n),
        }
    }

    fn test(db_str: String, query_str: String, expect_regs: Vec<Cell>, expect_res: bool) {
        let db_clauses = crate::parse::database(&db_str).unwrap();
        let (_, query_terms) = crate::parse::query(&query_str).unwrap();
        let db = crate::compile_db::compile_db(db_clauses);
        let query = crate::compile_query::compile_query(query_terms);
        let all_instructions = compile_link::compile_link(query, db);
        let (regs, result) = execute_instructions(all_instructions);
        assert_eq!(result, expect_res);
        let normalized_actual: Vec<Cell> = regs
            .get_arg_registers()
            .iter()
            .map(normalize_cell)
            .collect();
        let normalized_expected: Vec<Cell> = expect_regs.iter().map(normalize_cell).collect();
        assert_eq!(normalized_actual, normalized_expected);
    }
    fn pad_empties_to_32(regs: Vec<Cell>) -> Vec<Cell> {
        let len = regs.len();
        regs.into_iter()
            .chain(std::iter::repeat(Cell::new_var()).take(32 - len))
            .collect()
    }

    #[test]
    fn test_simple_put_atom() {
        test(
            "hello.".to_string(),
            "hello.".to_string(),
            pad_empties_to_32(vec![Cell::Struct {
                functor: "hello".to_string(),
                arity: 0,
                children: vec![],
            }]),
            true,
        );
        test(
            "hello.".to_string(),
            "bye.".to_string(),
            pad_empties_to_32(vec![Cell::Struct {
                functor: "bye".to_string(),
                arity: 0,
                children: vec![],
            }]),
            false,
        );
    }

    #[test]
    fn test_socrates_who() {
        test(
            r#"mortal(socrates)."#.to_string(),
            "mortal(X).".to_string(),
            pad_empties_to_32(vec![Cell::Struct {
                functor: "socrates".into(),
                arity: 0,
                children: vec![],
            }]),
            true,
        );
    }

    #[test]
    fn test_socrates_immortal() {
        test(
            r#"mortal(socrates)."#.to_string(),
            "mortal(dracle).".to_string(),
            pad_empties_to_32(vec![Cell::Struct {
                functor: "dracle".into(),
                arity: 0,
                children: vec![],
            }]),
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
