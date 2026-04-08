use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Ident(String),
    Number(u64),
    EqEq,
    LBracket,
    RBracket,
    LParen,
    RParen,
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}

#[derive(Debug, Error)]
#[error("lex error at position {pos}: {message}")]
pub struct LexError {
    pub pos: usize,
    pub message: String,
}

pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    let bytes = input.as_bytes();
    let mut i = 0usize;
    let mut tokens = Vec::new();

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        match ch {
            '=' => {
                if i + 1 < bytes.len() && bytes[i + 1] as char == '=' {
                    tokens.push(Token {
                        kind: TokenKind::EqEq,
                        pos: i,
                    });
                    i += 2;
                } else {
                    return Err(LexError {
                        pos: i,
                        message: "expected '=' after '='".to_string(),
                    });
                }
            }
            '[' => {
                tokens.push(Token {
                    kind: TokenKind::LBracket,
                    pos: i,
                });
                i += 1;
            }
            ']' => {
                tokens.push(Token {
                    kind: TokenKind::RBracket,
                    pos: i,
                });
                i += 1;
            }
            '(' => {
                tokens.push(Token {
                    kind: TokenKind::LParen,
                    pos: i,
                });
                i += 1;
            }
            ')' => {
                tokens.push(Token {
                    kind: TokenKind::RParen,
                    pos: i,
                });
                i += 1;
            }
            '0'..='9' => {
                let start = i;
                if i + 1 < bytes.len()
                    && bytes[i] as char == '0'
                    && matches!(bytes[i + 1] as char, 'x' | 'X')
                {
                    i += 2;
                    let hex_start = i;
                    while i < bytes.len() && (bytes[i] as char).is_ascii_hexdigit() {
                        i += 1;
                    }
                    if i == hex_start {
                        return Err(LexError {
                            pos: start,
                            message: "invalid hex literal".to_string(),
                        });
                    }
                    let value = u64::from_str_radix(&input[hex_start..i], 16).map_err(|_| LexError {
                        pos: start,
                        message: "hex literal out of range".to_string(),
                    })?;
                    tokens.push(Token {
                        kind: TokenKind::Number(value),
                        pos: start,
                    });
                } else {
                    while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                        i += 1;
                    }
                    let value = input[start..i].parse::<u64>().map_err(|_| LexError {
                        pos: start,
                        message: "number literal out of range".to_string(),
                    })?;
                    tokens.push(Token {
                        kind: TokenKind::Number(value),
                        pos: start,
                    });
                }
            }
            _ if ch.is_ascii_alphabetic() || ch == '_' => {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i] as char;
                    if c.is_ascii_alphanumeric() || c == '_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let raw = &input[start..i];
                let lower = raw.to_ascii_lowercase();
                let kind = match lower.as_str() {
                    "and" => TokenKind::And,
                    "or" => TokenKind::Or,
                    "not" => TokenKind::Not,
                    _ => TokenKind::Ident(raw.to_string()),
                };
                tokens.push(Token { kind, pos: start });
            }
            _ => {
                return Err(LexError {
                    pos: i,
                    message: format!("unexpected character '{ch}'"),
                });
            }
        }
    }

    Ok(tokens)
}
