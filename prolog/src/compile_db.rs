use std::collections::HashMap;

use crate::types::{Clause, Term, WamInstr, WamRegister};

#[allow(unused)]
struct RegisterManager {
    count: u32,
}

impl RegisterManager {
    fn new() -> Self {
        RegisterManager { count: 0 }
    }

    fn get_next(&mut self) -> u32 {
        let current = self.count;
        self.count += 1;
        current
    }

    fn reset(&mut self) {
        self.count = 0;
    }
}

struct ArgRegisterManager {
    inner: RegisterManager,
}

impl ArgRegisterManager {
    fn new() -> Self {
        ArgRegisterManager {
            inner: RegisterManager::new(),
        }
    }

    fn get_next(&mut self) -> WamRegister {
        WamRegister::A(self.inner.get_next())
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

struct XRegisterManager {
    inner: RegisterManager,
}

impl XRegisterManager {
    fn new() -> Self {
        XRegisterManager {
            inner: RegisterManager::new(),
        }
    }

    fn get_next(&mut self) -> WamRegister {
        WamRegister::X(self.inner.get_next())
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

#[allow(unused)]
pub(crate) struct Compiler {
    declared_functors: Vec<HashMap<String, u32>>,
    declared_vars: Vec<HashMap<String, WamRegister>>, // atomもここ
    arg_register_manager: ArgRegisterManager,
    x_register_manager: XRegisterManager,
}

#[allow(unused)]
impl Compiler {
    pub fn new() -> Self {
        Compiler {
            declared_functors: vec![],
            declared_vars: vec![],
            arg_register_manager: ArgRegisterManager::new(),
            x_register_manager: XRegisterManager::new(),
        }
    }

    fn find_functor(&self, functor: &str) -> Option<u32> {
        for scope in self.declared_functors.iter().rev() {
            if let Some(arity) = scope.get(functor) {
                return Some(*arity);
            }
        }
        None
    }
    fn decl_functor(&mut self, functor: String, arity: u32) {
        if let Some(scope) = self.declared_functors.last_mut() {
            scope.insert(functor, arity);
        } else {
            panic!("No scope to declare variable");
        }
    }

    fn find_var(&self, var: &str) -> Option<&WamRegister> {
        for scope in self.declared_vars.iter().rev() {
            if let Some(reg) = scope.get(var) {
                return Some(reg);
            }
        }
        None
    }
    fn decl_var(&mut self, var: String, reg: WamRegister) {
        if let Some(scope) = self.declared_vars.last_mut() {
            scope.insert(var, reg);
        } else {
            panic!("No scope to declare variable");
        }
    }
    fn push_scope(&mut self) {
        self.declared_functors.push(HashMap::new());
        self.declared_vars.push(HashMap::new());
    }
    fn pop_scope(&mut self) {
        self.declared_functors.pop();
        self.declared_vars.pop();
    }
    fn get_next_a(&mut self) -> WamRegister {
        self.arg_register_manager.get_next()
    }

    fn get_next_x(&mut self) -> WamRegister {
        self.x_register_manager.get_next()
    }

    fn cleanup_regs(&mut self) {
        self.arg_register_manager.reset();
        self.x_register_manager.reset();
    }

    pub fn compile_db(&mut self, db: Vec<Clause>) -> Vec<WamInstr> {
        db.into_iter()
            .flat_map(|clause| match clause {
                Clause::Fact(term) => self.compile_db_term(term),
                Clause::Rule { head, body } => {
                    todo!();
                }
            })
            .collect()
    }

    fn compile_db_term(&mut self, term: Term) -> Vec<WamInstr> {
        self.push_scope();
        let res = self.compile_db_term_impl(term);
        self.cleanup_regs();
        res
    }

    fn compile_db_term_impl(&mut self, term: Term) -> Vec<WamInstr> {
        match term {
            Term::Number(val) => {
                vec![WamInstr::UnifyNumber { val }]
            }
            Term::Atom(name) => {
                if let Some(&reg) = self.find_var(&name) {
                    vec![WamInstr::UnifyAtom { reg }]
                } else {
                    let reg = self.get_next_a();
                    self.decl_var(name.clone(), reg);
                    vec![WamInstr::GetAtom { name, reg }]
                }
            }
            Term::Var(name) => {
                if let Some(&reg) = self.find_var(&name) {
                    vec![WamInstr::UnifyVar { reg }]
                } else {
                    let reg = self.get_next_a();
                    self.decl_var(name.clone(), reg);
                    vec![WamInstr::GetVar { name, reg }] // TODO Xレジスタの宣言？必要な場合がわかってない
                }
            }
            _ => {
                todo!()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::clause;
    use crate::types::WamRegister;

    fn test_compile_db(source: &str, expected: Vec<WamInstr>) {
        let mut compiler = Compiler::new();
        let parsed_clause = clause(source).unwrap().1;
        let instructions = compiler.compile_db(vec![parsed_clause]);
        assert_eq!(instructions, expected);
    }

    #[test]
    fn register_managers() {
        let mut compiler = Compiler::new();

        // Test A register allocation
        let a_reg1 = compiler.get_next_a();
        let a_reg2 = compiler.get_next_a();
        assert_eq!(a_reg1, WamRegister::A(0));
        assert_eq!(a_reg2, WamRegister::A(1));

        // Test X register allocation
        let x_reg1 = compiler.get_next_x();
        let x_reg2 = compiler.get_next_x();
        assert_eq!(x_reg1, WamRegister::X(0));
        assert_eq!(x_reg2, WamRegister::X(1));

        // Test reset
        compiler.cleanup_regs();
        let a_reg_after_reset = compiler.get_next_a();
        let x_reg_after_reset = compiler.get_next_x();
        assert_eq!(a_reg_after_reset, WamRegister::A(0));
        assert_eq!(x_reg_after_reset, WamRegister::X(0));
    }

    #[test]
    fn db_atom() {
        test_compile_db(
            "parent.",
            vec![WamInstr::GetAtom {
                name: "parent".to_string(),
                reg: WamRegister::A(0),
            }],
        );
    }

    #[test]
    fn db_clause() {
        test_compile_db(
            "parent(john, doe).",
            vec![
                WamInstr::GetStruct {
                    functor: "parent".to_string(),
                    arity: 2,
                    reg: WamRegister::A(0),
                },
                WamInstr::GetAtom {
                    name: "john".to_string(),
                    reg: WamRegister::A(1),
                },
                WamInstr::GetAtom {
                    name: "doe".to_string(),
                    reg: WamRegister::A(2),
                },
            ],
        );
    }

    #[test]
    fn db_clause_var_shared() {
        test_compile_db(
            "a(X, X).",
            vec![
                WamInstr::GetStruct {
                    functor: "a".to_string(),
                    arity: 2,
                    reg: WamRegister::A(0),
                },
                WamInstr::GetVar {
                    name: "X".to_string(),
                    reg: WamRegister::A(1),
                },
                WamInstr::UnifyVar {
                    reg: WamRegister::A(1),
                },
            ],
        );
    }
}
