use std::net::Ipv4Addr;
use std::ops::{BitOr, BitOrAssign};

use thiserror::Error;

use crate::parser::{Expr, PacketWidth, Predicate, Value};
use crate::semantics::{SemanticError, SemanticInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerMask(u8);

impl LayerMask {
    const VALID_BITS: u8 = 0b0001_1111;

    pub const NETWORK: Self = Self(0b0000_0001);
    pub const NETWORK_FORWARD: Self = Self(0b0000_0010);
    pub const FLOW: Self = Self(0b0000_0100);
    pub const SOCKET: Self = Self(0b0000_1000);
    pub const REFLECT: Self = Self(0b0001_0000);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    fn from_bits(bits: u8) -> Result<Self, DecodeError> {
        if bits & !Self::VALID_BITS != 0 {
            return Err(DecodeError::InvalidLayerMask(bits));
        }
        Ok(Self(bits))
    }
}

impl BitOr for LayerMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for LayerMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpCode {
    FieldTest { field: &'static str, value: u64 },
    PacketLoad32 { offset: u16, value: u32 },
    PacketLoad16 { offset: u16, value: u16 },
    PacketLoad8 { offset: u16, value: u8 },
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterIr {
    pub required_layers: LayerMask,
    pub needs_payload: bool,
    pub referenced_fields: Vec<&'static str>,
    pub program: Vec<OpCode>,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("invalid WDIR magic")]
    InvalidMagic,
    #[error("unsupported WDIR version {0}")]
    UnsupportedVersion(u8),
    #[error("truncated WDIR payload")]
    Truncated,
    #[error("invalid layer mask 0x{0:02x}")]
    InvalidLayerMask(u8),
    #[error("invalid utf-8 in WDIR payload")]
    InvalidUtf8,
    #[error("unsupported field '{0}' in WDIR payload")]
    UnsupportedField(String),
    #[error("unsupported opcode {0} in WDIR payload")]
    UnsupportedOpcode(u8),
    #[error("too many referenced fields in WDIR payload: {0}")]
    TooManyReferencedFields(usize),
    #[error("WDIR program too long: {0}")]
    ProgramTooLong(usize),
    #[error("field byte length exceeds WDIR limit: {0}")]
    FieldByteLengthTooLarge(usize),
}

const MAX_REFERENCED_FIELDS: usize = 256;
const MAX_PROGRAM_LEN: usize = 4096;
const MAX_FIELD_BYTE_LEN: usize = 32;

pub fn encode_ir(ir: &FilterIr) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"WDIR");
    out.push(1);
    out.push(ir.required_layers.bits());
    out.push(u8::from(ir.needs_payload));

    write_u16(&mut out, ir.referenced_fields.len() as u16);
    for field in &ir.referenced_fields {
        let bytes = field.as_bytes();
        write_u16(&mut out, bytes.len() as u16);
        out.extend_from_slice(bytes);
    }

    write_u32(&mut out, ir.program.len() as u32);
    for op in &ir.program {
        match op {
            OpCode::FieldTest { field, value } => {
                out.push(1);
                let bytes = field.as_bytes();
                write_u16(&mut out, bytes.len() as u16);
                out.extend_from_slice(bytes);
                write_u64(&mut out, *value);
            }
            OpCode::PacketLoad32 { offset, value } => {
                out.push(2);
                write_u16(&mut out, *offset);
                write_u32(&mut out, *value);
            }
            OpCode::PacketLoad16 { offset, value } => {
                out.push(7);
                write_u16(&mut out, *offset);
                write_u16(&mut out, *value);
            }
            OpCode::PacketLoad8 { offset, value } => {
                out.push(3);
                write_u16(&mut out, *offset);
                out.push(*value);
            }
            OpCode::And => out.push(4),
            OpCode::Or => out.push(5),
            OpCode::Not => out.push(6),
        }
    }

    out
}

pub fn decode_ir(bytes: &[u8]) -> Result<FilterIr, DecodeError> {
    let mut reader = Reader::new(bytes);
    if reader.read_exact(4)? != b"WDIR" {
        return Err(DecodeError::InvalidMagic);
    }

    let version = reader.read_u8()?;
    if version != 1 {
        return Err(DecodeError::UnsupportedVersion(version));
    }

    let required_layers = LayerMask::from_bits(reader.read_u8()?)?;
    let needs_payload = match reader.read_u8()? {
        0 => false,
        _ => true,
    };

    let referenced_fields_len = reader.read_u16()? as usize;
    if referenced_fields_len > MAX_REFERENCED_FIELDS {
        return Err(DecodeError::TooManyReferencedFields(referenced_fields_len));
    }
    let mut referenced_fields = Vec::with_capacity(referenced_fields_len);
    for _ in 0..referenced_fields_len {
        let field = decode_field(reader.read_bytes(MAX_FIELD_BYTE_LEN)?)?;
        referenced_fields.push(field);
    }

    let program_len = reader.read_u32()? as usize;
    if program_len > MAX_PROGRAM_LEN {
        return Err(DecodeError::ProgramTooLong(program_len));
    }
    let mut program = Vec::with_capacity(program_len);
    for _ in 0..program_len {
        let opcode = reader.read_u8()?;
        let op = match opcode {
            1 => {
                let field = decode_field(reader.read_bytes(MAX_FIELD_BYTE_LEN)?)?;
                let value = reader.read_u64()?;
                OpCode::FieldTest { field, value }
            }
            2 => OpCode::PacketLoad32 {
                offset: reader.read_u16()?,
                value: reader.read_u32()?,
            },
            7 => OpCode::PacketLoad16 {
                offset: reader.read_u16()?,
                value: reader.read_u16()?,
            },
            3 => OpCode::PacketLoad8 {
                offset: reader.read_u16()?,
                value: reader.read_u8()?,
            },
            4 => OpCode::And,
            5 => OpCode::Or,
            6 => OpCode::Not,
            other => return Err(DecodeError::UnsupportedOpcode(other)),
        };
        program.push(op);
    }

    if !reader.is_empty() {
        return Err(DecodeError::UnsupportedOpcode(reader.read_u8()?));
    }

    Ok(FilterIr {
        required_layers,
        needs_payload,
        referenced_fields,
        program,
    })
}

pub(crate) fn lower(expr: &Expr, semantic: SemanticInfo) -> Result<FilterIr, SemanticError> {
    let mut program = Vec::new();
    lower_expr(expr, &mut program)?;
    Ok(FilterIr {
        required_layers: semantic.required_layers,
        needs_payload: semantic.needs_payload,
        referenced_fields: semantic.referenced_fields,
        program,
    })
}

fn lower_expr(expr: &Expr, program: &mut Vec<OpCode>) -> Result<(), SemanticError> {
    match expr {
        Expr::And(l, r) => {
            lower_expr(l, program)?;
            lower_expr(r, program)?;
            program.push(OpCode::And);
        }
        Expr::Or(l, r) => {
            lower_expr(l, program)?;
            lower_expr(r, program)?;
            program.push(OpCode::Or);
        }
        Expr::Not(inner) => {
            lower_expr(inner, program)?;
            program.push(OpCode::Not);
        }
        Expr::Predicate(p) => lower_predicate(p, program)?,
    }
    Ok(())
}

fn lower_predicate(p: &Predicate, program: &mut Vec<OpCode>) -> Result<(), SemanticError> {
    match p {
        Predicate::BareSymbol(symbol) => {
            let field = map_bool_symbol(symbol)?;
            program.push(OpCode::FieldTest { field, value: 1 });
        }
        Predicate::FieldEq { field, value } => {
            let field_name = map_field(field)?;
            let val = map_value(field_name, value)?;
            program.push(OpCode::FieldTest {
                field: field_name,
                value: val,
            });
        }
        Predicate::PacketEq {
            width,
            offset,
            value,
        } => match width {
            PacketWidth::Dword => {
                let v = u32::try_from(*value).map_err(|_| {
                    SemanticError::from_message("packet32 value out of range for u32")
                })?;
                program.push(OpCode::PacketLoad32 {
                    offset: *offset,
                    value: v,
                });
            }
            PacketWidth::Word => {
                let v = u16::try_from(*value).map_err(|_| {
                    SemanticError::from_message("packet16 value out of range for u16")
                })?;
                program.push(OpCode::PacketLoad16 {
                    offset: *offset,
                    value: v,
                });
            }
            PacketWidth::Byte => {
                let v = u8::try_from(*value).map_err(|_| {
                    SemanticError::from_message("packet value out of range for u8")
                })?;
                program.push(OpCode::PacketLoad8 {
                    offset: *offset,
                    value: v,
                });
            }
        },
    }
    Ok(())
}

fn map_bool_symbol(symbol: &str) -> Result<&'static str, SemanticError> {
    match symbol.to_ascii_lowercase().as_str() {
        "tcp" => Ok("tcp"),
        "udp" => Ok("udp"),
        "ipv4" => Ok("ipv4"),
        "ipv6" => Ok("ipv6"),
        "outbound" => Ok("outbound"),
        "inbound" => Ok("inbound"),
        _ => Err(SemanticError::from_message(format!(
            "unsupported symbol '{}'",
            symbol
        ))),
    }
}

fn map_field(field: &str) -> Result<&'static str, SemanticError> {
    match field.to_ascii_lowercase().as_str() {
        "event" => Ok("event"),
        "layer" => Ok("layer"),
        "processid" => Ok("processId"),
        "tcp" => Ok("tcp"),
        "udp" => Ok("udp"),
        "ipv4" => Ok("ipv4"),
        "ipv6" => Ok("ipv6"),
        "localaddr" => Ok("localAddr"),
        "remoteaddr" => Ok("remoteAddr"),
        "localport" => Ok("localPort"),
        "remoteport" => Ok("remotePort"),
        "outbound" => Ok("outbound"),
        "inbound" => Ok("inbound"),
        _ => Err(SemanticError::from_message(format!(
            "unsupported field '{}'",
            field
        ))),
    }
}

fn map_value(field: &'static str, value: &Value) -> Result<u64, SemanticError> {
    match (field, value) {
        ("processId", Value::Number(n)) => Ok(*n),
        (_, Value::Number(n)) => Ok(*n),
        ("event", Value::Symbol(s)) if s.eq_ignore_ascii_case("open") => Ok(1),
        ("event", Value::Symbol(s)) if s.eq_ignore_ascii_case("connect") => Ok(2),
        ("event", Value::Symbol(s)) if s.eq_ignore_ascii_case("close") => Ok(3),
        ("event", Value::Symbol(s)) if s.eq_ignore_ascii_case("established") => Ok(4),
        ("layer", Value::Symbol(s)) if s.eq_ignore_ascii_case("network") => Ok(1),
        ("layer", Value::Symbol(s)) if s.eq_ignore_ascii_case("network_forward") => Ok(2),
        ("layer", Value::Symbol(s)) if s.eq_ignore_ascii_case("flow") => Ok(3),
        ("layer", Value::Symbol(s)) if s.eq_ignore_ascii_case("socket") => Ok(4),
        ("layer", Value::Symbol(s)) if s.eq_ignore_ascii_case("reflect") => Ok(5),
        ("localAddr" | "remoteAddr", Value::Symbol(s)) => {
            parse_ipv4_match_value(s).map_err(|_| {
                SemanticError::from_message(format!(
                    "unsupported symbolic value '{}' for field '{}'",
                    s, field
                ))
            })
        }
        (_, Value::Symbol(s)) => Err(SemanticError::from_message(format!(
            "unsupported symbolic value '{}' for field '{}'",
            s, field
        ))),
    }
}

fn parse_ipv4_match_value(raw: &str) -> Result<u64, String> {
    if let Some((addr, prefix)) = raw.split_once('/') {
        let addr = addr
            .parse::<Ipv4Addr>()
            .map_err(|_| "invalid ipv4 cidr literal".to_string())?;
        let prefix = prefix
            .parse::<u8>()
            .map_err(|_| "invalid ipv4 cidr prefix".to_string())?;
        if prefix > 32 {
            return Err("invalid ipv4 cidr prefix".to_string());
        }
        let mask = ipv4_prefix_mask(prefix);
        let network = u32::from(addr) & mask;
        return Ok((u64::from(prefix) << 32) | u64::from(network));
    }

    let addr = raw
        .parse::<Ipv4Addr>()
        .map_err(|_| "invalid ipv4 literal".to_string())?;
    Ok((u64::from(32u8) << 32) | u64::from(u32::from(addr)))
}

fn ipv4_prefix_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    }
}

fn decode_field(bytes: &[u8]) -> Result<&'static str, DecodeError> {
    let field = std::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
    match field {
        "event" => Ok("event"),
        "layer" => Ok("layer"),
        "processId" => Ok("processId"),
        "tcp" => Ok("tcp"),
        "udp" => Ok("udp"),
        "ipv4" => Ok("ipv4"),
        "ipv6" => Ok("ipv6"),
        "localAddr" => Ok("localAddr"),
        "remoteAddr" => Ok("remoteAddr"),
        "localPort" => Ok("localPort"),
        "remotePort" => Ok("remotePort"),
        "outbound" => Ok("outbound"),
        "inbound" => Ok("inbound"),
        "packet" => Ok("packet"),
        _ => Err(DecodeError::UnsupportedField(field.to_string())),
    }
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.offset.checked_add(len).ok_or(DecodeError::Truncated)?;
        let slice = self.bytes.get(self.offset..end).ok_or(DecodeError::Truncated)?;
        self.offset = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, DecodeError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, DecodeError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, DecodeError> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_bytes(&mut self, max_len: usize) -> Result<&'a [u8], DecodeError> {
        let len = self.read_u16()? as usize;
        if len > max_len {
            return Err(DecodeError::FieldByteLengthTooLarge(len));
        }
        self.read_exact(len)
    }
}
