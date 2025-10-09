use std::collections::HashMap;

use crate::types::{Clause, Term, WamInstr, WamRegister};

#[allow(unused)]
pub(crate) struct Compiler {
    declared_functors: Vec<HashMap<String, u64>>,
    declared_vars: Vec<HashMap<String, WamRegister>>,
}

#[allow(unused)]
impl Compiler {
    pub fn new() -> Self {
        Compiler {
            declared_functors: vec![],
            declared_vars: vec![],
        }
    }

    pub fn compile_db(&self, db: Vec<Clause>) -> Vec<WamInstr> {
        db.into_iter()
            .flat_map(|clause| match clause {
                Clause::Fact(term) => self.compile_db_term(term),
                Clause::Rule { head, body } => {
                    todo!();
                }
            })
            .collect()
    }

    fn compile_db_term(&self, term: Term) -> Vec<WamInstr> {
        match term {
            Term::Number(val) => {
                vec![WamInstr::UnifyNumber { val }]
            }
            Term::Atom(name) => {
                // TODO 宣言済みならUnify, していなければGet
                vec![WamInstr::UnifyAtom { name }]
            }
            Term::Var(name) => {
                // TODO 宣言済みならUnify, していなければGet
                vec![WamInstr::UnifyVar { name: name.into() }]
            }
            _ => {
                todo!()
            }
        }
    }

    pub fn compile_query(&self, query: Vec<Clause>) -> Vec<WamInstr> {
        todo!()
    }
    fn find_functor(&self, functor: &str) -> Option<u64> {
        for scope in self.declared_functors.iter().rev() {
            if let Some(arity) = scope.get(functor) {
                return Some(*arity);
            }
        }
        None
    }
    fn decl_functor(&mut self, functor: String, arity: u64) {
        if let Some(scope) = self.declared_functors.last_mut() {
            scope.insert(functor, arity);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::clause;
    use crate::parse::term;
    use crate::types::WamRegister;

    #[test]
    fn db_atom() {
        let compiler = Compiler::new();
        let term = term("parent.").unwrap().1;
        let instructions = compiler.compile_db_term(term);
        assert_eq!(
            instructions,
            vec![WamInstr::GetAtom {
                name: "parent".to_string(),
                reg: WamRegister::A(0)
            },]
        );
    }

    #[test]
    fn db_clause() {
        let compiler = Compiler::new();
        let clause_str = "parent(john, doe).";
        let clause = clause(clause_str).unwrap().1;
        let instructions = compiler.compile_db(vec![clause]);
        assert_eq!(
            instructions,
            vec![
                WamInstr::GetStruct {
                    functor: "parent".to_string(),
                    arity: 2,
                    reg: WamRegister::A(0)
                },
                WamInstr::GetAtom {
                    name: "john".to_string(),
                    reg: WamRegister::A(1)
                },
                WamInstr::GetAtom {
                    name: "doe".to_string(),
                    reg: WamRegister::A(2),
                },
            ]
        );
    }
}
