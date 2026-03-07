use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::parse::{Clause, Term, database};

#[derive(Debug)]
pub enum ModuleError {
    FileNotFound {
        module_path: String,
        searched: Vec<PathBuf>,
    },
    CyclicDependency {
        path: PathBuf,
    },
    ParseError {
        path: PathBuf,
        message: String,
    },
    IoError {
        path: PathBuf,
        error: std::io::Error,
    },
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleError::FileNotFound {
                module_path,
                searched,
            } => {
                write!(f, "Module '{}' not found. Searched:", module_path)?;
                for p in searched {
                    write!(f, "\n  {}", p.display())?;
                }
                Ok(())
            }
            ModuleError::CyclicDependency { path } => {
                write!(f, "Cyclic dependency detected: {}", path.display())
            }
            ModuleError::ParseError { path, message } => {
                write!(f, "Parse error in {}: {}", path.display(), message)
            }
            ModuleError::IoError { path, error } => {
                write!(f, "IO error reading {}: {}", path.display(), error)
            }
        }
    }
}

impl std::error::Error for ModuleError {}

pub fn resolve_modules(
    clauses: Vec<Clause>,
    include_paths: &[PathBuf],
    visited: &mut HashSet<PathBuf>,
) -> Result<Vec<Clause>, ModuleError> {
    let mut result = Vec::new();

    for clause in clauses {
        match clause {
            Clause::Use { path, expose, .. } => {
                let resolved = resolve_use(&path, &expose, include_paths, visited)?;
                result.extend(resolved);
            }
            other => result.push(other),
        }
    }

    Ok(result)
}

fn find_module_file(module_path: &str, include_paths: &[PathBuf]) -> Option<PathBuf> {
    let file_name = format!("{}.cadhr", module_path);
    for dir in include_paths {
        let candidate = dir.join(&file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn module_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn resolve_use(
    module_path: &str,
    expose: &[String],
    include_paths: &[PathBuf],
    visited: &mut HashSet<PathBuf>,
) -> Result<Vec<Clause>, ModuleError> {
    let file_path = find_module_file(module_path, include_paths).ok_or_else(|| {
        ModuleError::FileNotFound {
            module_path: module_path.to_string(),
            searched: include_paths
                .iter()
                .map(|p| p.join(format!("{}.cadhr", module_path)))
                .collect(),
        }
    })?;

    let canonical = file_path
        .canonicalize()
        .map_err(|e| ModuleError::IoError {
            path: file_path.clone(),
            error: e,
        })?;

    if !visited.insert(canonical.clone()) {
        return Err(ModuleError::CyclicDependency { path: canonical });
    }

    let source =
        std::fs::read_to_string(&file_path).map_err(|e| ModuleError::IoError {
            path: file_path.clone(),
            error: e,
        })?;

    let clauses = database(&source).map_err(|e| ModuleError::ParseError {
        path: file_path.clone(),
        message: format!("{:?}", e),
    })?;

    let child_include_paths: Vec<PathBuf> = file_path
        .parent()
        .map(|p| vec![p.to_path_buf()])
        .unwrap_or_default();

    let clauses = resolve_modules(clauses, &child_include_paths, visited)?;

    let module_name = module_name_from_path(module_path);
    let expose_set: HashSet<&str> = expose.iter().map(|s| s.as_str()).collect();

    let mut result = Vec::new();
    for clause in clauses {
        let prefixed = prefix_clause(&clause, &module_name);
        result.push(prefixed);

        if let Some(functor) = clause_head_functor(&clause) {
            if expose_set.contains(functor.as_str()) {
                result.push(clause);
            }
        }
    }

    Ok(result)
}

fn clause_head_functor(clause: &Clause) -> Option<String> {
    match clause {
        Clause::Fact(term) => term_functor(term),
        Clause::Rule { head, .. } => term_functor(head),
        Clause::Use { .. } => None,
    }
}

fn term_functor(term: &Term) -> Option<String> {
    match term {
        Term::Struct { functor, .. } => Some(functor.clone()),
        _ => None,
    }
}

fn prefix_clause(clause: &Clause, module_name: &str) -> Clause {
    match clause {
        Clause::Fact(term) => Clause::Fact(prefix_term(term, module_name)),
        Clause::Rule { head, body } => Clause::Rule {
            head: prefix_term(head, module_name),
            body: body.iter().map(|t| prefix_term(t, module_name)).collect(),
        },
        Clause::Use { .. } => clause.clone(),
    }
}

fn prefix_term(term: &Term, module_name: &str) -> Term {
    match term {
        Term::Struct {
            functor,
            args,
            span,
        } => Term::Struct {
            functor: format!("{}::{}", module_name, functor),
            args: args.iter().map(|a| prefix_term(a, module_name)).collect(),
            span: *span,
        },
        Term::List { items, tail } => Term::List {
            items: items.iter().map(|i| prefix_term(i, module_name)).collect(),
            tail: tail.as_ref().map(|t| Box::new(prefix_term(t, module_name))),
        },
        Term::InfixExpr { op, left, right } => Term::InfixExpr {
            op: *op,
            left: Box::new(prefix_term(left, module_name)),
            right: Box::new(prefix_term(right, module_name)),
        },
        Term::Constraint { left, right } => Term::Constraint {
            left: Box::new(prefix_term(left, module_name)),
            right: Box::new(prefix_term(right, module_name)),
        },
        _ => term.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_basic_module_resolution() {
        let dir = setup_test_dir();
        fs::write(
            dir.path().join("bolts.cadhr"),
            "m5(X) :- size(X, 5).\nsize(small, 3).\n",
        )
        .unwrap();

        let clauses = vec![Clause::Use {
            path: "bolts".to_string(),
            expose: vec![],
            span: None,
        }];

        let mut visited = HashSet::new();
        let result =
            resolve_modules(clauses, &[dir.path().to_path_buf()], &mut visited).unwrap();

        assert!(result
            .iter()
            .any(|c| matches!(c, Clause::Rule { head, .. }
                if matches!(head, Term::Struct { functor, .. } if functor == "bolts::m5"))));
        assert!(result
            .iter()
            .any(|c| matches!(c, Clause::Fact(Term::Struct { functor, .. })
                if functor == "bolts::size")));
    }

    #[test]
    fn test_expose() {
        let dir = setup_test_dir();
        fs::write(dir.path().join("bolts.cadhr"), "m5(5).\nm6(6).\n").unwrap();

        let clauses = vec![Clause::Use {
            path: "bolts".to_string(),
            expose: vec!["m5".to_string()],
            span: None,
        }];

        let mut visited = HashSet::new();
        let result =
            resolve_modules(clauses, &[dir.path().to_path_buf()], &mut visited).unwrap();

        // bolts::m5 と m5 の両方が存在する
        assert!(result
            .iter()
            .any(|c| matches!(c, Clause::Fact(Term::Struct { functor, .. })
                if functor == "bolts::m5")));
        assert!(result
            .iter()
            .any(|c| matches!(c, Clause::Fact(Term::Struct { functor, .. })
                if functor == "m5")));
        // m6 は expose されていないので非修飾版は無い
        assert!(!result
            .iter()
            .any(|c| matches!(c, Clause::Fact(Term::Struct { functor, .. })
                if functor == "m6")));
        assert!(result
            .iter()
            .any(|c| matches!(c, Clause::Fact(Term::Struct { functor, .. })
                if functor == "bolts::m6")));
    }

    #[test]
    fn test_cyclic_dependency() {
        let dir = setup_test_dir();
        fs::write(dir.path().join("a.cadhr"), "#use(\"b\").\nfoo(1).\n").unwrap();
        fs::write(dir.path().join("b.cadhr"), "#use(\"a\").\nbar(2).\n").unwrap();

        let clauses = vec![Clause::Use {
            path: "a".to_string(),
            expose: vec![],
            span: None,
        }];

        let mut visited = HashSet::new();
        let result = resolve_modules(clauses, &[dir.path().to_path_buf()], &mut visited);
        assert!(matches!(result, Err(ModuleError::CyclicDependency { .. })));
    }

    #[test]
    fn test_file_not_found() {
        let dir = setup_test_dir();

        let clauses = vec![Clause::Use {
            path: "nonexistent".to_string(),
            expose: vec![],
            span: None,
        }];

        let mut visited = HashSet::new();
        let result = resolve_modules(clauses, &[dir.path().to_path_buf()], &mut visited);
        assert!(matches!(result, Err(ModuleError::FileNotFound { .. })));
    }

    #[test]
    fn test_nested_module() {
        let dir = setup_test_dir();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/parts.cadhr"), "bolt(1).\n").unwrap();

        let clauses = vec![Clause::Use {
            path: "sub/parts".to_string(),
            expose: vec![],
            span: None,
        }];

        let mut visited = HashSet::new();
        let result =
            resolve_modules(clauses, &[dir.path().to_path_buf()], &mut visited).unwrap();

        assert!(result
            .iter()
            .any(|c| matches!(c, Clause::Fact(Term::Struct { functor, .. })
                if functor == "parts::bolt")));
    }

    #[test]
    fn test_non_use_clauses_preserved() {
        let clauses = vec![
            Clause::Fact(Term::Struct {
                functor: "hello".to_string(),
                args: vec![],
                span: None,
            }),
        ];

        let mut visited = HashSet::new();
        let result = resolve_modules(clauses, &[], &mut visited).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], Clause::Fact(Term::Struct { functor, .. }) if functor == "hello"));
    }
}
