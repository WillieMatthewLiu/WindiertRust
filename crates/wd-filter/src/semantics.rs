use std::collections::BTreeSet;

use thiserror::Error;

use crate::ir::LayerMask;
use crate::parser::{Expr, Predicate, Value};

#[derive(Debug, Clone)]
pub(crate) struct SemanticInfo {
    pub required_layers: LayerMask,
    pub needs_payload: bool,
    pub referenced_fields: Vec<&'static str>,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct SemanticError {
    message: String,
}

impl SemanticError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn from_message(message: impl Into<String>) -> Self {
        Self::new(message)
    }
}

pub(crate) fn analyze(expr: &Expr) -> Result<SemanticInfo, SemanticError> {
    let mut state = State {
        required_layers: LayerMask::empty(),
        needs_payload: false,
        referenced_fields: BTreeSet::new(),
        layer_flow_selected: false,
        packet_access_used: false,
    };
    visit(expr, &mut state, false, true)?;

    if state.layer_flow_selected && state.packet_access_used {
        return Err(SemanticError::new(
            "packet access is not valid for FLOW",
        ));
    }

    Ok(SemanticInfo {
        required_layers: state.required_layers,
        needs_payload: state.needs_payload,
        referenced_fields: state.referenced_fields.into_iter().collect(),
    })
}

struct State {
    required_layers: LayerMask,
    needs_payload: bool,
    referenced_fields: BTreeSet<&'static str>,
    layer_flow_selected: bool,
    packet_access_used: bool,
}

fn visit(
    expr: &Expr,
    state: &mut State,
    reflect_context: bool,
    polarity: bool,
) -> Result<(), SemanticError> {
    match expr {
        Expr::And(l, r) => {
            let left_ctx = reflect_context || contains_positive_reflect_open(r, polarity);
            let right_ctx = reflect_context || contains_positive_reflect_open(l, polarity);
            visit(l, state, left_ctx, polarity)?;
            visit(r, state, right_ctx, polarity)?;
        }
        Expr::Or(l, r) => {
            visit(l, state, reflect_context, polarity)?;
            visit(r, state, reflect_context, polarity)?;
        }
        Expr::Not(inner) => visit(inner, state, reflect_context, !polarity)?,
        Expr::Predicate(p) => visit_predicate(p, state, reflect_context, polarity)?,
    }
    Ok(())
}

fn visit_predicate(
    p: &Predicate,
    state: &mut State,
    reflect_context: bool,
    polarity: bool,
) -> Result<(), SemanticError> {
    match p {
        Predicate::BareSymbol(symbol) => match symbol.to_ascii_lowercase().as_str() {
            "tcp" => {
                state.referenced_fields.insert("tcp");
            }
            "udp" => {
                state.referenced_fields.insert("udp");
            }
            "ipv4" | "ipv6" => {
                if polarity {
                    state.required_layers.insert(LayerMask::NETWORK);
                    state.required_layers.insert(LayerMask::NETWORK_FORWARD);
                }
                state.referenced_fields.insert(if symbol.eq_ignore_ascii_case("ipv4") {
                    "ipv4"
                } else {
                    "ipv6"
                });
            }
            "outbound" => {
                if polarity {
                    state.required_layers.insert(LayerMask::NETWORK_FORWARD);
                }
                state.referenced_fields.insert("outbound");
            }
            "inbound" => {
                if polarity {
                    state.required_layers.insert(LayerMask::NETWORK);
                }
                state.referenced_fields.insert("inbound");
            }
            other => {
                return Err(SemanticError::new(format!(
                    "unsupported symbol '{other}'"
                )));
            }
        },
        Predicate::FieldEq { field, value } => {
            let field_name = canonical_field(field).ok_or_else(|| {
                SemanticError::new(format!("unsupported field '{}'", field))
            })?;
            state.referenced_fields.insert(field_name);

            if field_name == "event" {
                match value {
                    Value::Symbol(sym) if sym.eq_ignore_ascii_case("open") => {
                        if polarity {
                            state.required_layers.insert(LayerMask::REFLECT);
                        }
                    }
                    Value::Symbol(sym) if sym.eq_ignore_ascii_case("connect") => {
                        if polarity {
                            state.required_layers.insert(LayerMask::SOCKET);
                        }
                    }
                    Value::Symbol(sym) if sym.eq_ignore_ascii_case("close") => {
                        if polarity {
                            state.required_layers.insert(LayerMask::REFLECT);
                        }
                    }
                    Value::Symbol(sym) => {
                        return Err(SemanticError::new(format!(
                            "unsupported event '{sym}'"
                        )));
                    }
                    Value::Number(_) => {
                        return Err(SemanticError::new(
                            "event comparison expects symbolic value",
                        ));
                    }
                }
            }

            if field_name == "layer" {
                match value {
                    Value::Symbol(sym) if sym.eq_ignore_ascii_case("flow") => {
                        if polarity && !reflect_context {
                            state.layer_flow_selected = true;
                            state.required_layers.insert(LayerMask::FLOW);
                        }
                    }
                    Value::Symbol(sym) if sym.eq_ignore_ascii_case("network") => {
                        if polarity && !reflect_context {
                            state.required_layers.insert(LayerMask::NETWORK);
                        }
                    }
                    Value::Symbol(sym) if sym.eq_ignore_ascii_case("network_forward") => {
                        if polarity && !reflect_context {
                            state.required_layers.insert(LayerMask::NETWORK_FORWARD);
                        }
                    }
                    Value::Symbol(sym) if sym.eq_ignore_ascii_case("socket") => {
                        if polarity && !reflect_context {
                            state.required_layers.insert(LayerMask::SOCKET);
                        }
                    }
                    Value::Symbol(sym) if sym.eq_ignore_ascii_case("reflect") => {
                        if polarity && !reflect_context {
                            state.required_layers.insert(LayerMask::REFLECT);
                        }
                    }
                    Value::Symbol(sym) => {
                        return Err(SemanticError::new(format!(
                            "unsupported layer '{sym}'"
                        )));
                    }
                    Value::Number(_) => {
                        return Err(SemanticError::new(
                            "layer comparison expects symbolic value",
                        ));
                    }
                }
            }

            if field_name == "processId" {
                match value {
                    Value::Number(_) => {
                        if polarity {
                            state.required_layers.insert(LayerMask::SOCKET);
                        }
                    }
                    Value::Symbol(sym) => {
                        return Err(SemanticError::new(format!(
                            "processId comparison expects numeric value, got '{sym}'"
                        )));
                    }
                }
            }

            if matches!(field_name, "localPort" | "remotePort" | "localAddr" | "remoteAddr")
                && polarity
            {
                state.required_layers.insert(LayerMask::NETWORK);
                state.required_layers.insert(LayerMask::NETWORK_FORWARD);
            }
        }
        Predicate::PacketEq { .. } => {
            state.packet_access_used = true;
            state.needs_payload = true;
            state.required_layers.insert(LayerMask::NETWORK);
            state.referenced_fields.insert("packet");
        }
    }
    Ok(())
}

fn canonical_field(field: &str) -> Option<&'static str> {
    match field.to_ascii_lowercase().as_str() {
        "event" => Some("event"),
        "layer" => Some("layer"),
        "processid" => Some("processId"),
        "tcp" => Some("tcp"),
        "udp" => Some("udp"),
        "ipv4" => Some("ipv4"),
        "ipv6" => Some("ipv6"),
        "localaddr" => Some("localAddr"),
        "remoteaddr" => Some("remoteAddr"),
        "localport" => Some("localPort"),
        "remoteport" => Some("remotePort"),
        "outbound" => Some("outbound"),
        "inbound" => Some("inbound"),
        _ => None,
    }
}

fn contains_positive_reflect_open(expr: &Expr, polarity: bool) -> bool {
    match expr {
        Expr::And(l, r) | Expr::Or(l, r) => {
            contains_positive_reflect_open(l, polarity)
                || contains_positive_reflect_open(r, polarity)
        }
        Expr::Not(inner) => contains_positive_reflect_open(inner, !polarity),
        Expr::Predicate(Predicate::FieldEq { field, value }) => {
            polarity
                && field.eq_ignore_ascii_case("event")
                && matches!(value, Value::Symbol(sym) if sym.eq_ignore_ascii_case("open"))
        }
        Expr::Predicate(_) => false,
    }
}
