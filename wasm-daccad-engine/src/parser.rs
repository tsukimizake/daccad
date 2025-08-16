use nom::combinator::opt;
use nom::error as ne;
use nom::error::ErrorKind;
use nom::{character::complete::space0, combinator::recognize};
use nom::{
    branch::alt,
    bytes::complete::{take_while, take_while1},
    character::complete::char,
    combinator::map,
    multi::many0,
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};
use nom_locate::LocatedSpan;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use elm_rs::{Elm, ElmDecode, ElmEncode};
use tsify::Tsify;

use super::env::{Env, ModelId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct Value {
    #[wasm_bindgen(skip)]
    pub inner: ValueInner,
}

#[derive(Debug, Clone, Serialize, Deserialize, Elm, ElmDecode, ElmEncode, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(tag = "t", content = "c")]
pub enum ValueInner {
    Integer(i64),
    Double(f64),
    String(String),
    Symbol(String),
    Stl(ModelId),
    List(Vec<ValueInner>),
}

#[wasm_bindgen]
impl Value {
    #[wasm_bindgen(constructor)]
    pub fn new_integer(value: i64) -> Value {
        Value {
            inner: ValueInner::Integer(value),
        }
    }

    #[wasm_bindgen]
    pub fn new_double(value: f64) -> Value {
        Value {
            inner: ValueInner::Double(value),
        }
    }

    #[wasm_bindgen]
    pub fn new_string(value: String) -> Value {
        Value {
            inner: ValueInner::String(value),
        }
    }
}

pub fn cast_evaled(expr: Expr) -> ValueInner {
    match expr {
        Expr::Integer { value, .. } => ValueInner::Integer(value),
        Expr::Double { value, .. } => ValueInner::Double(value),
        Expr::Model { id, .. } => ValueInner::Stl(id),
        Expr::String { value, .. } => ValueInner::String(value),
        Expr::Symbol { name, .. } => ValueInner::Symbol(name),
        Expr::List { elements, .. } => ValueInner::List(elements.into_iter().map(cast_evaled).collect()),
        Expr::Quote { expr, .. } => cast_evaled(*expr),
        Expr::Quasiquote { expr, .. } => cast_evaled(*expr),
        Expr::Unquote { expr, .. } => cast_evaled(*expr),
        Expr::Builtin { name, .. } => ValueInner::Symbol(format!("<builtin {}>", name)),
        Expr::SpecialForm { name, .. } => ValueInner::Symbol(format!("<special form {}>", name)),
        Expr::Clausure { .. } => ValueInner::Symbol("<closure>".to_string()),
        Expr::Macro { .. } => ValueInner::Symbol("<macro>".to_string()),
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Symbol {
        name: String,
        location: Option<usize>,
        trailing_newline: bool,
    },
    List {
        elements: Vec<Expr>,
        location: Option<usize>,
        trailing_newline: bool,
    },
    Integer {
        value: i64,
        location: Option<usize>,
        trailing_newline: bool,
    },
    String {
        value: String,
        location: Option<usize>,
        trailing_newline: bool,
    },
    Double {
        value: f64,
        location: Option<usize>,
        trailing_newline: bool,
    },
    Model {
        id: ModelId,
        location: Option<usize>,
        trailing_newline: bool,
    },
    Quote {
        expr: Box<Expr>,
        location: Option<usize>,
        trailing_newline: bool,
    },
    Quasiquote {
        expr: Box<Expr>,
        location: Option<usize>,
        trailing_newline: bool,
    },
    Unquote {
        expr: Box<Expr>,
        location: Option<usize>,
        trailing_newline: bool,
    },
    Builtin {
        name: String,
        fun: fn(&[Expr], &mut Env) -> Result<Expr, String>,
    },
    SpecialForm {
        name: String,
        fun: fn(&[Expr], &mut Env) -> Result<Expr, String>,
    },
    Clausure {
        args: Vec<String>,
        body: Box<Expr>,
    },
    Macro {
        args: Vec<String>,
        body: Box<Expr>,
    },
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        use Expr::*;
        match (self, other) {
            (
                Symbol {
                    name: n1,
                    location: loc1,
                    trailing_newline: tn1,
                },
                Symbol {
                    name: n2,
                    location: loc2,
                    trailing_newline: tn2,
                },
            ) => n1 == n2 && loc1 == loc2 && tn1 == tn2,
            (
                List {
                    elements: e1,
                    location: loc1,
                    trailing_newline: tn1,
                },
                List {
                    elements: e2,
                    location: loc2,
                    trailing_newline: tn2,
                },
            ) => e1 == e2 && loc1 == loc2 && tn1 == tn2,
            (
                Integer {
                    value: v1,
                    location: loc1,
                    trailing_newline: tn1,
                },
                Integer {
                    value: v2,
                    location: loc2,
                    trailing_newline: tn2,
                },
            ) => v1 == v2 && loc1 == loc2 && tn1 == tn2,
            (
                Double {
                    value: v1,
                    location: loc1,
                    trailing_newline: tn1,
                },
                Double {
                    value: v2,
                    location: loc2,
                    trailing_newline: tn2,
                },
            ) => v1 == v2 && loc1 == loc2 && tn1 == tn2,
            (
                String {
                    value: v1,
                    location: loc1,
                    trailing_newline: tn1,
                },
                String {
                    value: v2,
                    location: loc2,
                    trailing_newline: tn2,
                },
            ) => v1 == v2 && loc1 == loc2 && tn1 == tn2,
            (
                Model {
                    id: id1,
                    location: loc1,
                    trailing_newline: tn1,
                },
                Model {
                    id: id2,
                    location: loc2,
                    trailing_newline: tn2,
                },
            ) => id1 == id2 && loc1 == loc2 && tn1 == tn2,
            (
                Quote {
                    expr: e1,
                    location: loc1,
                    trailing_newline: tn1,
                },
                Quote {
                    expr: e2,
                    location: loc2,
                    trailing_newline: tn2,
                },
            ) => e1 == e2 && loc1 == loc2 && tn1 == tn2,
            (Builtin { name: n1, .. }, Builtin { name: n2, .. }) => n1 == n2,
            (SpecialForm { name: n1, .. }, SpecialForm { name: n2, .. }) => n1 == n2,
            _ => false,
        }
    }
}

impl Expr {
    pub fn symbol(name: &str) -> Self {
        Expr::Symbol {
            name: name.to_string(),
            location: None,
            trailing_newline: false,
        }
    }
    
    pub fn integer(value: i64) -> Self {
        Expr::Integer {
            value,
            location: None,
            trailing_newline: false,
        }
    }
    
    pub fn double(value: f64) -> Self {
        Expr::Double {
            value,
            location: None,
            trailing_newline: false,
        }
    }
    
    pub fn string(value: String) -> Self {
        Expr::String {
            value,
            location: None,
            trailing_newline: false,
        }
    }
    
    pub fn model(id: ModelId) -> Self {
        Expr::Model {
            id,
            location: None,
            trailing_newline: false,
        }
    }
    
    pub fn list(elements: Vec<Expr>) -> Self {
        Expr::List {
            elements,
            location: None,
            trailing_newline: false,
        }
    }

    pub fn nil() -> Self {
        Self::list(vec![])
    }

    pub fn is_symbol(&self, name: &str) -> bool {
        match self {
            Expr::Symbol { name: n, .. } => n == name,
            _ => false,
        }
    }

    pub fn as_symbol(&self) -> Result<&str, String> {
        match self {
            Expr::Symbol { name, .. } => Ok(name),
            _ => Err("Not a symbol".to_string()),
        }
    }

    pub fn set_newline(self: Self, b: bool) -> Self {
        match self {
            Expr::Symbol { name, location, .. } => Expr::Symbol {
                name,
                location,
                trailing_newline: b,
            },
            Expr::List {
                elements, location, ..
            } => Expr::List {
                elements,
                location,
                trailing_newline: b,
            },
            Expr::Integer {
                value, location, ..
            } => Expr::Integer {
                value,
                location,
                trailing_newline: b,
            },
            Expr::Double {
                value, location, ..
            } => Expr::Double {
                value,
                location,
                trailing_newline: b,
            },
            Expr::String {
                value, location, ..
            } => Expr::String {
                value,
                location,
                trailing_newline: b,
            },
            Expr::Model { id, location, .. } => Expr::Model {
                id,
                location,
                trailing_newline: b,
            },
            Expr::Quote { expr, location, .. } => Expr::Quote {
                expr,
                location,
                trailing_newline: b,
            },
            Expr::Quasiquote { expr, location, .. } => Expr::Quasiquote {
                expr,
                location,
                trailing_newline: b,
            },
            Expr::Unquote { expr, location, .. } => Expr::Unquote {
                expr,
                location,
                trailing_newline: b,
            },
            _ => self,
        }
    }
}

pub fn parse_file(input: &str) -> Result<Vec<Expr>, String> {
    match tokenize(LocatedSpan::new(input)) {
        Ok((_, tokens)) => {
            let mut exprs = vec![];
            let mut rest = &tokens[..];
            while !rest.is_empty() {
                match expr(rest) {
                    Ok((new_rest, expr)) => {
                        exprs.push(expr);
                        rest = new_rest;
                    }
                    Err(e) => return Err(format!("Error: {:?}", e)),
                }
            }
            Ok(exprs)
        }
        Err(e) => Err(format!("Error: {:?}", e)),
    }
}

pub type Span<'a> = LocatedSpan<&'a str>;

#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {
    Symbol(Span<'a>),
    Integer(Span<'a>),
    Double(Span<'a>),
    Quote(Span<'a>),
    Quasiquote(Span<'a>),
    Unquote(Span<'a>),
    String(Span<'a>),
    LParen(Span<'a>),
    RParen(Span<'a>),
    Newline(Span<'a>),
    Comment(Span<'a>),
}

fn symbol(input: Span) -> IResult<Span, Token> {
    map(
        take_while1(|c: char| c.is_alphanumeric() || "_+-*/<>#?!.".contains(c)),
        Token::Symbol,
    )(input)
}

fn integer(input: Span) -> IResult<Span, Token> {
    map(
        recognize(pair(opt(char('-')), take_while1(|c: char| c.is_digit(10)))),
        |span: Span| Token::Integer(span),
    )(input)
}

fn double(input: Span) -> IResult<Span, Token> {
    map(
        recognize(pair(
            opt(char('-')),
            pair(
                take_while1(|c: char| c.is_digit(10)),
                preceded(char('.'), take_while1(|c: char| c.is_digit(10))),
            ),
        )),
        |span: Span| Token::Double(span),
    )(input)
}

fn string(input: Span) -> IResult<Span, Token> {
    map(
        delimited(char('"'), take_while1(|c: char| c != '"'), char('"')),
        Token::String,
    )(input)
}

fn quote(input: Span) -> IResult<Span, Token> {
    map(char('\''), |_| Token::Quote(input))(input)
}

fn quasiquote(input: Span) -> IResult<Span, Token> {
    map(char('`'), |_| Token::Quasiquote(input))(input)
}

fn unquote(input: Span) -> IResult<Span, Token> {
    map(char('~'), |_| Token::Unquote(input))(input)
}

fn lparen(input: Span) -> IResult<Span, Token> {
    map(char('('), |_| Token::LParen(input))(input)
}

fn rparen(input: Span) -> IResult<Span, Token> {
    map(char(')'), |_| Token::RParen(input))(input)
}

fn newline(input: Span) -> IResult<Span, Token> {
    map(char('\n'), |_| Token::Newline(input))(input)
}

fn comment(input: Span) -> IResult<Span, Token> {
    let (input, _) = char(';')(input)?;
    let (input, content) = take_while(|c| c != '\n')(input)?;
    Ok((input, Token::Comment(content)))
}

fn tokenize(input: Span) -> IResult<Span, Vec<Token>> {
    let (input, all_tokens) = many0(delimited(
        space0,
        alt((
            string, double, integer, symbol, quote, quasiquote, unquote, lparen, rparen, newline,
            comment,
        )),
        space0,
    ))(input)?;

    let tokens = all_tokens
        .into_iter()
        .filter(|token| !matches!(token, Token::Comment(_)))
        .collect();

    Ok((input, tokens))
}

fn expr<'a>(tokens: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    tuple((
        alt((
            parse_string,
            parse_double,
            parse_integer,
            parse_symbol,
            parse_quote,
            parse_quasiquote,
            parse_unquote,
            parse_list,
        )),
        many0(parse_newline),
    ))(tokens)
    .map(|(input, (expr, newlines))| {
        if !newlines.is_empty() {
            (input, expr.set_newline(true))
        } else {
            (input, expr)
        }
    })
}

fn parse_symbol<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    if let Some((Token::Symbol(span), rest)) = input.split_first() {
        Ok((
            rest,
            Expr::Symbol {
                name: span.fragment().to_string(),
                location: Some(span.location_offset()),
                trailing_newline: false,
            },
        ))
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}

fn parse_integer<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    if let Some((Token::Integer(span), rest)) = input.split_first() {
        Ok((
            rest,
            Expr::Integer {
                value: span.fragment().parse().unwrap(),
                location: Some(span.location_offset()),
                trailing_newline: false,
            },
        ))
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}

fn parse_double<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    if let Some((Token::Double(span), rest)) = input.split_first() {
        Ok((
            rest,
            Expr::Double {
                value: span.fragment().parse().unwrap(),
                location: Some(span.location_offset()),
                trailing_newline: false,
            },
        ))
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}

fn parse_string<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    if let Some((Token::String(span), rest)) = input.split_first() {
        Ok((
            rest,
            Expr::String {
                value: span.fragment().to_string(),
                location: Some(span.location_offset()),
                trailing_newline: false,
            },
        ))
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}

fn parse_quote<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    if let Some((Token::Quote(span), rest)) = input.split_first() {
        match expr(rest) {
            Ok((rest, expr)) => Ok((
                rest,
                Expr::Quote {
                    expr: Box::new(expr),
                    location: Some(span.location_offset()),
                    trailing_newline: false,
                },
            )),
            Err(e) => Err(e),
        }
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}

fn parse_quasiquote<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    if let Some((Token::Quasiquote(span), rest)) = input.split_first() {
        match expr(rest) {
            Ok((rest, expr)) => Ok((
                rest,
                Expr::Quasiquote {
                    expr: Box::new(expr),
                    location: Some(span.location_offset()),
                    trailing_newline: false,
                },
            )),
            Err(e) => Err(e),
        }
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}

fn parse_unquote<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    if let Some((Token::Unquote(span), rest)) = input.split_first() {
        match expr(rest) {
            Ok((rest, expr)) => Ok((
                rest,
                Expr::Unquote {
                    expr: Box::new(expr),
                    location: Some(span.location_offset()),
                    trailing_newline: false,
                },
            )),
            Err(e) => Err(e),
        }
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}

fn parse_list<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Expr> {
    if let Some((Token::LParen(span), rest)) = input.split_first() {
        let mut elements = vec![];
        let mut rest = rest;
        while let Ok((new_rest, expr)) = expr(rest) {
            elements.push(expr);
            rest = new_rest;
        }
        if let Some((Token::RParen(_), rest)) = rest.split_first() {
            Ok((
                rest,
                Expr::List {
                    elements,
                    location: Some(span.location_offset()),
                    trailing_newline: false,
                },
            ))
        } else {
            Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
        }
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}

fn parse_newline<'a>(input: &'a [Token]) -> IResult<&'a [Token<'a>], Token<'a>> {
    if let Some((Token::Newline(span), rest)) = input.split_first() {
        Ok((rest, Token::Newline(*span)))
    } else {
        Err(nom::Err::Error(ne::Error::new(input, ErrorKind::Tag)))
    }
}