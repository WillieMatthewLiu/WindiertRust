use thiserror::Error;

use crate::lexer::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Predicate(Predicate),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PacketWidth {
    Byte,
    Word,
    Dword,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Value {
    Number(u64),
    Symbol(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Predicate {
    BareSymbol(String),
    FieldEq { field: String, value: Value },
    PacketEq {
        width: PacketWidth,
        offset: u16,
        value: u64,
    },
}

#[derive(Debug, Error)]
#[error("parse error at token {pos}: {message}")]
pub struct ParseError {
    pub pos: usize,
    pub message: String,
}

pub fn parse(tokens: &[Token]) -> Result<Expr, ParseError> {
    let mut p = Parser { tokens, idx: 0 };
    let expr = p.parse_or()?;
    if p.idx != tokens.len() {
        return Err(ParseError {
            pos: tokens[p.idx].pos,
            message: "unexpected trailing token".to_string(),
        });
    }
    Ok(expr)
}

struct Parser<'a> {
    tokens: &'a [Token],
    idx: usize,
}

impl<'a> Parser<'a> {
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while self.match_kind(&TokenKind::Or) {
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not()?;
        while self.match_kind(&TokenKind::And) {
            let right = self.parse_not()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        if self.match_kind(&TokenKind::Not) {
            return Ok(Expr::Not(Box::new(self.parse_not()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        if self.match_kind(&TokenKind::LParen) {
            let inner = self.parse_or()?;
            self.expect_kind(&TokenKind::RParen, "expected ')'")?;
            return Ok(inner);
        }

        let field_tok = self.next().ok_or(ParseError {
            pos: self.last_pos(),
            message: "expected expression".to_string(),
        })?;
        let field = match &field_tok.kind {
            TokenKind::Ident(s) => s.clone(),
            _ => {
                return Err(ParseError {
                    pos: field_tok.pos,
                    message: "expected identifier".to_string(),
                });
            }
        };

        if self.match_kind(&TokenKind::LBracket) {
            let offset = self.expect_number("expected packet offset")?;
            let offset = u16::try_from(offset).map_err(|_| ParseError {
                pos: field_tok.pos,
                message: "packet offset is out of range".to_string(),
            })?;
            self.expect_kind(&TokenKind::RBracket, "expected ']'")?;
            self.expect_kind(&TokenKind::EqEq, "expected '=='")?;
            let value = self.expect_number("expected packet value")?;
            let width = match field.to_ascii_lowercase().as_str() {
                "packet" => PacketWidth::Byte,
                "packet16" => PacketWidth::Word,
                "packet32" => PacketWidth::Dword,
                _ => {
                    return Err(ParseError {
                        pos: field_tok.pos,
                        message: format!("unsupported packet accessor '{field}'"),
                    });
                }
            };
            return Ok(Expr::Predicate(Predicate::PacketEq {
                width,
                offset,
                value,
            }));
        }

        if self.match_kind(&TokenKind::EqEq) {
            let value_tok = self.next().ok_or(ParseError {
                pos: field_tok.pos,
                message: "expected value after '=='".to_string(),
            })?;
            let value = match &value_tok.kind {
                TokenKind::Number(n) => Value::Number(*n),
                TokenKind::Ident(s) => Value::Symbol(s.clone()),
                _ => {
                    return Err(ParseError {
                        pos: value_tok.pos,
                        message: "expected number or symbol after '=='".to_string(),
                    });
                }
            };
            return Ok(Expr::Predicate(Predicate::FieldEq { field, value }));
        }

        Ok(Expr::Predicate(Predicate::BareSymbol(field)))
    }

    fn expect_number(&mut self, message: &str) -> Result<u64, ParseError> {
        let tok = self.next().ok_or(ParseError {
            pos: self.last_pos(),
            message: message.to_string(),
        })?;
        match tok.kind {
            TokenKind::Number(n) => Ok(n),
            _ => Err(ParseError {
                pos: tok.pos,
                message: message.to_string(),
            }),
        }
    }

    fn match_kind(&mut self, kind: &TokenKind) -> bool {
        if let Some(tok) = self.peek() {
            if token_kind_eq(&tok.kind, kind) {
                self.idx += 1;
                return true;
            }
        }
        false
    }

    fn expect_kind(&mut self, kind: &TokenKind, message: &str) -> Result<(), ParseError> {
        let tok = self.next().ok_or(ParseError {
            pos: self.last_pos(),
            message: message.to_string(),
        })?;
        if token_kind_eq(&tok.kind, kind) {
            Ok(())
        } else {
            Err(ParseError {
                pos: tok.pos,
                message: message.to_string(),
            })
        }
    }

    fn next(&mut self) -> Option<&'a Token> {
        let tok = self.tokens.get(self.idx)?;
        self.idx += 1;
        Some(tok)
    }

    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.idx)
    }

    fn last_pos(&self) -> usize {
        self.tokens
            .get(self.idx.saturating_sub(1))
            .map(|t| t.pos)
            .unwrap_or(0)
    }
}

fn token_kind_eq(a: &TokenKind, b: &TokenKind) -> bool {
    matches!(
        (a, b),
        (TokenKind::EqEq, TokenKind::EqEq)
            | (TokenKind::LBracket, TokenKind::LBracket)
            | (TokenKind::RBracket, TokenKind::RBracket)
            | (TokenKind::LParen, TokenKind::LParen)
            | (TokenKind::RParen, TokenKind::RParen)
            | (TokenKind::And, TokenKind::And)
            | (TokenKind::Or, TokenKind::Or)
            | (TokenKind::Not, TokenKind::Not)
    )
}
