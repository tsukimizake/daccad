use std::rc::Rc;

use crate::compiler_bytecode::WamInstr;

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum HeapCell {
    Ref(Rc<HeapCell>),
    Struct { functor: String, arity: usize },
    Atom(String),
    Number(i64),
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum RegStackCell {
    Ref(Rc<HeapCell>),
    Struct { functor: String, arity: usize },
    Atom(String),
    Number(i64),
}

#[allow(unused)]
enum Frame {
    Base {},
    Environment {
        prev_ep: Rc<Frame>,
        return_pc: Rc<WamInstr>,
        registers: Vec<RegStackCell>,
    },

    ChoicePoint {
        // TODO
    },
}

#[allow(unused)]
struct TrailEntry {
    cells_to_revert: Vec<RegStackCell>,
}

#[allow(unused)]
struct Machine<'a> {
    heap: Vec<Rc<RegStackCell>>, // Hレジスタはheap.len()
    stack: Vec<Rc<Frame>>,
    arg_registers: Vec<Rc<RegStackCell>>, // TODO runtime_sized_arrayにする可能性
    other_registers: Vec<Rc<RegStackCell>>,
    program: &'a [WamInstr],
    current_instr: &'a WamInstr, // program counter相当
    env_p: Rc<Frame>,            // 現在の環境フレーム先頭
    choice_p: Rc<Frame>,         // 現在の選択ポイントフレーム先頭
    trail: Vec<TrailEntry>,
}

#[allow(unused)]
impl<'a> Machine<'a> {
    pub(super) fn new(program: &'a [WamInstr]) -> Self {
        let stack_head = Rc::new(Frame::Base {});
        let stack = vec![stack_head.clone()];
        Self {
            heap: Vec::with_capacity(32),
            stack: stack,
            arg_registers: Vec::with_capacity(32),
            other_registers: Vec::with_capacity(32),
            program,
            current_instr: &program[0],
            env_p: stack_head.clone(),
            choice_p: stack_head,
            trail: Vec::with_capacity(32),
        }
    }
    fn step(&mut self) {
        match self.current_instr {
            WamInstr::PutAtom { name, reg } => {
                let cell = Rc::new(RegStackCell::Atom(name.clone()));
                self.arg_registers.push(cell);
            }
            _ => {
                todo!();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::compile_db;

    use super::*;

    fn test(db_str: String, query_str: String, expect_todo: ()) {
        let (_, db_clause) = crate::parse::clause(&db_str).unwrap();
        let (_, query_term) = crate::parse::term(&query_str).unwrap();
        let db = compile_db::Compiler::new().compile_db(vec![db_clause]);
        let _query = crate::compile_query::Compiler::new().compile(query_term);
        let mut machine = Machine::new(&db);
        // 以下TODO
        machine.step();
        let expected = Rc::new(RegStackCell::Atom("hello.".to_string()));
        assert_eq!(machine.arg_registers[0], expected);
    }
    #[test]
    fn test_simple_put_atom() {
        test("hello.".to_string(), "hello.".to_string(), ());
    }

    #[allow(unused)]
    //TODO #[test]
    fn test_socrates() {
        test(
            r#"human(socrates). mortal(X) :- human(X)."#.to_string(),
            "mortal(socrates).".to_string(),
            (),
        );
    }
}
