mod ir;
mod lexer;
mod parser;
mod semantics;

pub use ir::{decode_ir, encode_ir, DecodeError, FilterIr, LayerMask, OpCode};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("{0}")]
    Lex(#[from] lexer::LexError),
    #[error("{0}")]
    Parse(#[from] parser::ParseError),
    #[error("{0}")]
    Semantic(#[from] semantics::SemanticError),
}

pub fn compile(input: &str) -> Result<FilterIr, CompileError> {
    let tokens = lexer::lex(input)?;
    let expr = parser::parse(&tokens)?;
    let semantic = semantics::analyze(&expr)?;
    let ir = ir::lower(&expr, semantic)?;
    Ok(ir)
}
