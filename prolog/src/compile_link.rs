// compile_queryとcompile_dbの結果をつなげ、CallTempをCallにfinalizeする

use crate::compile_query::CompiledQuery;
use crate::compiler_bytecode::WamInstr;
use std::collections::HashMap;

pub fn compile_link(query: CompiledQuery, db_instructions: Vec<WamInstr>) -> CompiledQuery {
    let mut label_to_line: HashMap<(String, usize), usize> = HashMap::new();

    let all_instructions = query
        .instructions
        .into_iter()
        .chain(db_instructions.into_iter())
        .enumerate()
        .map(|(line_num, instr)| {
            if let WamInstr::Label { name, arity } = &instr {
                label_to_line.insert((name.clone(), *arity), line_num);
                instr
            } else {
                instr
            }
        })
        .collect::<Vec<WamInstr>>();
    let linked = all_instructions
        .into_iter()
        .map(|instr| match instr {
            WamInstr::CallTemp { predicate, arity } => {
                if let Some(&target_line) = label_to_line.get(&(predicate.clone(), arity)) {
                    WamInstr::Call {
                        predicate,
                        arity,
                        to_program_counter: target_line,
                    }
                } else {
                    WamInstr::Error {
                        message: format!("error: predicate {} not found", predicate),
                    }
                }
            }
            other => other,
        })
        .collect();
    CompiledQuery {
        instructions: linked,
        term_to_reg: query.term_to_reg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_bytecode::{WamInstr, WamReg};
    use std::collections::HashMap;

    #[test]
    fn test_compile_link_basic() {
        let db_instructions = vec![
            WamInstr::Label {
                name: "parent".to_string(),
                arity: 2,
            },
            WamInstr::GetStruct {
                functor: "john".to_string(),
                arity: 0,
                reg: WamReg::X(0),
            },
            WamInstr::Proceed,
        ];

        let query = CompiledQuery {
            instructions: vec![
                WamInstr::PutStruct {
                    functor: "john".to_string(),
                    arity: 0,
                    reg: WamReg::X(0),
                },
                WamInstr::CallTemp {
                    predicate: "parent".to_string(),
                    arity: 2,
                },
            ],
            term_to_reg: HashMap::new(),
        };

        let result = compile_link(query, db_instructions);

        let expected = vec![
            WamInstr::PutStruct {
                functor: "john".to_string(),
                arity: 0,
                reg: WamReg::X(0),
            },
            WamInstr::Call {
                predicate: "parent".to_string(),
                arity: 2,
                to_program_counter: 2,
            },
            WamInstr::Label {
                name: "parent".to_string(),
                arity: 2,
            },
            WamInstr::GetStruct {
                functor: "john".to_string(),
                arity: 0,
                reg: WamReg::X(0),
            },
            WamInstr::Proceed,
        ];

        assert_eq!(result.instructions, expected);
    }
}
