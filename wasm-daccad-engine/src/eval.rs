use super::env::{Env, extract};
use super::parser::{Expr, cast_evaled};
use elm_rs::{Elm, ElmDecode, ElmEncode};
use serde::{Deserialize, Serialize};
use tsify::Tsify;

use super::env::ModelId;

// Simplified evaluated result for WASM
#[derive(Debug, Clone, Serialize, Deserialize, Elm, ElmDecode, ElmEncode, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Evaled {
    pub value: super::parser::ValueInner,
    pub preview_list: Vec<ModelId>,
}

pub fn run_file(file: &str, env: &mut Env) -> Result<Evaled, String> {
    let exprs = super::parser::parse_file(file)?;
    eval_exprs(exprs, env)
}

pub fn eval_exprs(exprs: Vec<Expr>, env: &mut Env) -> Result<Evaled, String> {
    let mut last_result = Ok(Expr::list(vec![]));

    // Evaluate each expression in sequence
    for expr in &exprs {
        last_result = eval(expr.clone(), env);
    }

    let preview_list = env.preview_list();

    last_result.map(|expr| {
        let value_inner = cast_evaled(expr);
        Evaled {
            value: value_inner,
            preview_list,
        }
    })
}

pub fn eval(expr: Expr, env: &mut Env) -> Result<Expr, String> {
    match expr {
        Expr::Symbol { name, .. } => env
            .get(&name)
            .ok_or_else(|| format!("Undefined symbol: {}", name)),
        Expr::Integer { value, .. } => Ok(Expr::integer(value)),
        Expr::Double { value, .. } => Ok(Expr::double(value)),
        Expr::List { elements, .. } => eval_list(&elements, env),
        Expr::String { value, .. } => Ok(Expr::string(value)),
        Expr::Model { id, .. } => Ok(Expr::model(id)),
        Expr::Quote { expr, .. } => Ok(*expr),
        // For these types, we can just return the original expression
        Expr::Builtin { .. }
        | Expr::SpecialForm { .. }
        | Expr::Clausure { .. }
        | Expr::Macro { .. } => Ok(expr),
        _ => Err("Unsupported expression type".to_string()),
    }
}

fn eval_list(elements: &[Expr], env: &mut Env) -> Result<Expr, String> {
    if elements.is_empty() {
        return Ok(Expr::list(vec![]));
    }

    // Check for special forms first
    let first_elem = &elements[0];
    if first_elem.is_symbol("lambda") {
        return eval_lambda(elements, env);
    }
    if first_elem.is_symbol("define") {
        return eval_define(elements, env);
    }
    if first_elem.is_symbol("if") {
        return eval_if(elements, env);
    }
    if first_elem.is_symbol("let") {
        return eval_let(elements, env);
    }

    // For function calls, evaluate the function expression first
    let first = eval(elements[0].clone(), env)?;
    match first {
        Expr::Builtin { fun, .. } => {
            let args = &elements[1..];
            let evaled = eval_args(args, env)?;
            fun(&evaled, env)
        }
        Expr::SpecialForm { fun, .. } => {
            // For special forms, don't evaluate the arguments yet
            let args = &elements[1..];
            fun(args, env)
        }
        Expr::Clausure { args, body } => {
            let mut new_env = Env::make_child(env.clone());

            // Evaluate arguments and bind them
            for (arg, value) in args.iter().zip(elements.iter().skip(1)) {
                let val = eval(value.clone(), env)?;
                new_env.insert(arg.clone(), val);
            }

            eval(*body, &mut new_env)
        }
        _ => Err("First element of list is not a function or special form".to_string()),
    }
}

fn eval_define(elements: &[Expr], env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(elements, 3)?;
    match elements.get(1) {
        Some(Expr::List {
            elements: fn_and_args,
            ..
        }) => {
            // Function definition: (define (name args...) body)
            if fn_and_args.is_empty() {
                return Err("Function name cannot be empty".to_string());
            }

            let fn_name = match &fn_and_args[0] {
                Expr::Symbol { name, .. } => name.clone(),
                _ => return Err("Function name must be a symbol".to_string()),
            };

            let args: Result<Vec<String>, String> = fn_and_args[1..]
                .iter()
                .map(|arg| match arg {
                    Expr::Symbol { name, .. } => Ok(name.clone()),
                    _ => Err("Function argument must be a symbol".to_string()),
                })
                .collect();

            let args = args?;
            let body = Box::new(elements[2].clone());

            let lambda = Expr::Clausure { args, body };

            env.insert(fn_name, lambda.clone());
            Ok(lambda)
        }
        Some(Expr::Symbol { name, .. }) => {
            // Variable definition: (define name value)
            let value = eval(elements[2].clone(), env)?;
            env.insert(name.clone(), value.clone());
            Ok(value)
        }
        _ => Err("Second argument to define must be a symbol or list".to_string()),
    }
}

fn eval_lambda(elements: &[Expr], env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(elements, 3)?;

    let args = match &elements[1] {
        Expr::List { elements: args, .. } => {
            let mut arg_names = Vec::new();
            for arg in args {
                match arg {
                    Expr::Symbol { name, .. } => arg_names.push(name.clone()),
                    _ => return Err("Lambda argument must be a symbol".to_string()),
                }
            }
            arg_names
        }
        _ => return Err("Lambda arguments must be a list".to_string()),
    };

    let body = Box::new(elements[2].clone());

    Ok(Expr::Clausure { args, body })
}

fn eval_if(elements: &[Expr], env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(elements, 3..=4)?;

    let condition = eval(elements[1].clone(), env)?;
    let is_truthy = match condition {
        Expr::Symbol { name, .. } if name == "#f" => false,
        Expr::List { elements, .. } if elements.is_empty() => false,
        _ => true,
    };

    if is_truthy {
        eval(elements[2].clone(), env)
    } else if elements.len() > 3 {
        eval(elements[3].clone(), env)
    } else {
        Ok(Expr::symbol("#f"))
    }
}

fn eval_let(elements: &[Expr], env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(elements, 3)?;

    let bindings = match &elements[1] {
        Expr::List {
            elements: bindings, ..
        } => bindings,
        _ => return Err("Let bindings must be a list".to_string()),
    };

    let mut new_env = Env::make_child(env.clone());

    // Process each binding
    for binding in bindings {
        match binding {
            Expr::List {
                elements: bind_elems,
                ..
            } if bind_elems.len() == 2 => {
                let var_name = match &bind_elems[0] {
                    Expr::Symbol { name, .. } => name.clone(),
                    _ => return Err("Binding variable must be a symbol".to_string()),
                };

                let value = eval(bind_elems[1].clone(), env)?;
                new_env.insert(var_name, value);
            }
            _ => return Err("Each binding must be a list of two elements".to_string()),
        }
    }

    eval(elements[2].clone(), &mut new_env)
}

fn eval_args(args: &[Expr], env: &mut Env) -> Result<Vec<Expr>, String> {
    args.iter().map(|arg| eval(arg.clone(), env)).collect()
}

pub fn assert_arg_count<R>(args: &[Expr], count: R) -> Result<(), String>
where
    R: Into<ArgCount>,
{
    let count = count.into();
    let actual = args.len();

    match count {
        ArgCount::Exact(expected) => {
            if actual == expected {
                Ok(())
            } else {
                Err(format!("Expected {} arguments, got {}", expected, actual))
            }
        }
        ArgCount::Range(min, max) => {
            if actual >= min && actual <= max {
                Ok(())
            } else {
                Err(format!(
                    "Expected {}-{} arguments, got {}",
                    min, max, actual
                ))
            }
        }
        ArgCount::AtLeast(min) => {
            if actual >= min {
                Ok(())
            } else {
                Err(format!(
                    "Expected at least {} arguments, got {}",
                    min, actual
                ))
            }
        }
    }
}

#[derive(Debug)]
pub enum ArgCount {
    Exact(usize),
    Range(usize, usize),
    AtLeast(usize),
}

impl From<usize> for ArgCount {
    fn from(count: usize) -> Self {
        ArgCount::Exact(count)
    }
}

impl From<std::ops::Range<usize>> for ArgCount {
    fn from(range: std::ops::Range<usize>) -> Self {
        ArgCount::Range(range.start, range.end - 1)
    }
}

impl From<std::ops::RangeInclusive<usize>> for ArgCount {
    fn from(range: std::ops::RangeInclusive<usize>) -> Self {
        ArgCount::Range(*range.start(), *range.end())
    }
}

impl From<std::ops::RangeFrom<usize>> for ArgCount {
    fn from(range: std::ops::RangeFrom<usize>) -> Self {
        ArgCount::AtLeast(range.start)
    }
}

pub fn default_env() -> Env {
    let mut env = Env::new();

    // Add basic arithmetic operations
    env.insert(
        "+".to_string(),
        Expr::Builtin {
            name: "+".to_string(),
            fun: add,
        },
    );
    env.insert(
        "-".to_string(),
        Expr::Builtin {
            name: "-".to_string(),
            fun: sub,
        },
    );
    env.insert(
        "*".to_string(),
        Expr::Builtin {
            name: "*".to_string(),
            fun: mul,
        },
    );
    env.insert(
        "/".to_string(),
        Expr::Builtin {
            name: "/".to_string(),
            fun: div,
        },
    );

    // Add CAD primitive operations
    env.insert(
        "point".to_string(),
        Expr::Builtin {
            name: "point".to_string(),
            fun: super::manifold_primitives::point,
        },
    );
    env.insert(
        "p".to_string(),
        Expr::Builtin {
            name: "p".to_string(),
            fun: super::manifold_primitives::point,
        },
    );
    env.insert(
        "cube".to_string(),
        Expr::Builtin {
            name: "cube".to_string(),
            fun: super::manifold_primitives::cube,
        },
    );
    env.insert(
        "cylinder".to_string(),
        Expr::Builtin {
            name: "cylinder".to_string(),
            fun: super::manifold_primitives::cylinder,
        },
    );
    env.insert(
        "union".to_string(),
        Expr::Builtin {
            name: "union".to_string(),
            fun: super::manifold_primitives::union,
        },
    );
    env.insert(
        "subtract".to_string(),
        Expr::Builtin {
            name: "subtract".to_string(),
            fun: super::manifold_primitives::subtract,
        },
    );
    env.insert(
        "intersect".to_string(),
        Expr::Builtin {
            name: "intersect".to_string(),
            fun: super::manifold_primitives::intersect,
        },
    );
    env.insert(
        "translate".to_string(),
        Expr::Builtin {
            name: "translate".to_string(),
            fun: super::manifold_primitives::translate,
        },
    );
    env.insert(
        "preview".to_string(),
        Expr::Builtin {
            name: "preview".to_string(),
            fun: super::manifold_primitives::preview,
        },
    );

    env
}

// Basic arithmetic functions
fn add(args: &[Expr], _env: &mut Env) -> Result<Expr, String> {
    let mut result = 0.0;
    for arg in args {
        result += extract::number(arg)?;
    }

    // Return integer if result is whole number, otherwise double
    if result.fract() == 0.0 {
        Ok(Expr::integer(result as i64))
    } else {
        Ok(Expr::double(result))
    }
}

fn sub(args: &[Expr], _env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 1..)?;

    if args.len() == 1 {
        let val = extract::number(&args[0])?;
        return Ok(Expr::double(-val));
    }

    let mut result = extract::number(&args[0])?;
    for arg in &args[1..] {
        result -= extract::number(arg)?;
    }

    if result.fract() == 0.0 {
        Ok(Expr::integer(result as i64))
    } else {
        Ok(Expr::double(result))
    }
}

fn mul(args: &[Expr], _env: &mut Env) -> Result<Expr, String> {
    let mut result = 1.0;
    for arg in args {
        result *= extract::number(arg)?;
    }

    if result.fract() == 0.0 {
        Ok(Expr::integer(result as i64))
    } else {
        Ok(Expr::double(result))
    }
}

fn div(args: &[Expr], _env: &mut Env) -> Result<Expr, String> {
    assert_arg_count(args, 1..)?;

    if args.len() == 1 {
        let val = extract::number(&args[0])?;
        return Ok(Expr::double(1.0 / val));
    }

    let mut result = extract::number(&args[0])?;
    for arg in &args[1..] {
        let divisor = extract::number(arg)?;
        if divisor == 0.0 {
            return Err("Division by zero".to_string());
        }
        result /= divisor;
    }

    if result.fract() == 0.0 {
        Ok(Expr::integer(result as i64))
    } else {
        Ok(Expr::double(result))
    }
}
