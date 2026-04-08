use std::fmt::{Display, Formatter};

use wd_filter::{FilterIr, LayerMask, OpCode};
use wd_proto::Layer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventKind {
    SocketConnect,
    ReflectOpen,
    ReflectClose,
}

impl EventKind {
    const fn code(self) -> u64 {
        match self {
            Self::ReflectOpen => 1,
            Self::SocketConnect => 2,
            Self::ReflectClose => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverEvent {
    SocketConnect { process_id: u64 },
    ReflectOpen { handle_id: u64 },
    ReflectClose { handle_id: u64 },
}

impl DriverEvent {
    pub const fn socket_connect(process_id: u64) -> Self {
        Self::SocketConnect { process_id }
    }

    pub const fn reflect_open(handle_id: u64) -> Self {
        Self::ReflectOpen { handle_id }
    }

    pub const fn reflect_close(handle_id: u64) -> Self {
        Self::ReflectClose { handle_id }
    }

    const fn layer(self) -> Layer {
        match self {
            Self::SocketConnect { .. } => Layer::Socket,
            Self::ReflectOpen { .. } | Self::ReflectClose { .. } => Layer::Reflect,
        }
    }

    const fn event_kind(self) -> EventKind {
        match self {
            Self::SocketConnect { .. } => EventKind::SocketConnect,
            Self::ReflectOpen { .. } => EventKind::ReflectOpen,
            Self::ReflectClose { .. } => EventKind::ReflectClose,
        }
    }

    const fn process_id(self) -> Option<u64> {
        match self {
            Self::SocketConnect { process_id } => Some(process_id),
            Self::ReflectOpen { .. } | Self::ReflectClose { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterEngine {
    layer: Layer,
    ir: FilterIr,
}

impl FilterEngine {
    pub fn compile(layer: Layer, input: &str) -> Result<Self, FilterCompileError> {
        let ir = wd_filter::compile(input).map_err(FilterCompileError::from_display)?;
        let bytes = wd_filter::encode_ir(&ir);
        Self::from_ir_bytes(layer, &bytes)
    }

    pub fn from_ir_bytes(layer: Layer, input: &[u8]) -> Result<Self, FilterCompileError> {
        let ir = wd_filter::decode_ir(input).map_err(FilterCompileError::from_display)?;
        Self::from_ir(layer, ir)
    }

    fn from_ir(layer: Layer, ir: FilterIr) -> Result<Self, FilterCompileError> {
        validate_ir(layer, &ir)?;
        Ok(Self { layer, ir })
    }

    pub fn matches(&self, event: &DriverEvent) -> bool {
        if event.layer() != self.layer {
            return false;
        }
        evaluate_program(&self.ir.program, *event).unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterCompileError {
    msg: String,
}

impl FilterCompileError {
    fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }

    fn from_display(err: impl Display) -> Self {
        Self::new(err.to_string())
    }
}

impl Display for FilterCompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for FilterCompileError {}

fn validate_ir(layer: Layer, ir: &FilterIr) -> Result<(), FilterCompileError> {
    if !matches!(layer, Layer::Socket | Layer::Reflect) {
        return Err(FilterCompileError::new(format!(
            "unsupported layer for FilterEngine: {:?}",
            layer
        )));
    }

    let selected_layer = layer_mask(layer);
    if ir.required_layers != LayerMask::empty() && !ir.required_layers.contains(selected_layer) {
        return Err(FilterCompileError::new(format!(
            "incompatible filter for {:?}: requires different layer",
            layer
        )));
    }
    if ir.needs_payload {
        return Err(FilterCompileError::new(
            "payload predicates are not supported by FilterEngine",
        ));
    }

    for field in &ir.referenced_fields {
        validate_supported_field(layer, field, None)?;
    }

    for op in &ir.program {
        match op {
            OpCode::FieldTest { field, value } => {
                validate_supported_field(layer, field, Some(*value))?;
            }
            OpCode::PacketLoad32 { .. } | OpCode::PacketLoad8 { .. } => {
                return Err(FilterCompileError::new(
                    "unsupported opcode for FilterEngine runtime subset: packet load",
                ));
            }
            OpCode::And | OpCode::Or | OpCode::Not => {}
        }
    }

    validate_program_shape(&ir.program)?;
    Ok(())
}

fn validate_supported_field(
    layer: Layer,
    field: &str,
    value: Option<u64>,
) -> Result<(), FilterCompileError> {
    match (layer, field, value) {
        (Layer::Socket, "event", Some(v)) if v != EventKind::SocketConnect.code() => {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Socket: event",
            ))
        }
        (Layer::Reflect, "event", Some(v))
            if v != EventKind::ReflectOpen.code() && v != EventKind::ReflectClose.code() =>
        {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Reflect: event",
            ))
        }
        (Layer::Socket, "event", _) | (Layer::Reflect, "event", _) => Ok(()),
        (Layer::Socket, "processId", _) => Ok(()),
        (Layer::Reflect, "processId", _) => Err(FilterCompileError::new(
            "unsupported field for layer Reflect: processId",
        )),
        (Layer::Socket, "layer", Some(v)) if v != layer_value(Layer::Socket) => {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Socket: layer",
            ))
        }
        (Layer::Reflect, "layer", Some(v)) if v != layer_value(Layer::Reflect) => {
            Err(FilterCompileError::new(
                "unsupported field/value combination for layer Reflect: layer",
            ))
        }
        (Layer::Socket, "layer", _) | (Layer::Reflect, "layer", _) => Ok(()),
        (_, "tcp", _) | (_, "inbound", _) => Err(FilterCompileError::new(format!(
            "unsupported field for FilterEngine runtime subset: {field}",
        ))),
        (_, other, _) => Err(FilterCompileError::new(format!(
            "unsupported field for FilterEngine runtime subset: {other}",
        ))),
    }
}

fn validate_program_shape(program: &[OpCode]) -> Result<(), FilterCompileError> {
    if program.is_empty() {
        return Err(FilterCompileError::new("filter program is empty"));
    }

    let mut depth = 0usize;
    for op in program {
        match op {
            OpCode::FieldTest { .. } | OpCode::PacketLoad32 { .. } | OpCode::PacketLoad8 { .. } => {
                depth += 1;
            }
            OpCode::Not => {
                if depth < 1 {
                    return Err(FilterCompileError::new("invalid filter program"));
                }
            }
            OpCode::And | OpCode::Or => {
                if depth < 2 {
                    return Err(FilterCompileError::new("invalid filter program"));
                }
                depth -= 1;
            }
        }
    }

    if depth != 1 {
        return Err(FilterCompileError::new("invalid filter program"));
    }

    Ok(())
}

fn evaluate_program(program: &[OpCode], event: DriverEvent) -> Option<bool> {
    let mut stack = Vec::with_capacity(program.len());
    for op in program {
        match op {
            OpCode::FieldTest { field, value } => stack.push(eval_field_test(event, field, *value)),
            OpCode::PacketLoad32 { .. } | OpCode::PacketLoad8 { .. } => return None,
            OpCode::And => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                stack.push(lhs && rhs);
            }
            OpCode::Or => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                stack.push(lhs || rhs);
            }
            OpCode::Not => {
                let value = stack.pop()?;
                stack.push(!value);
            }
        }
    }

    let result = stack.pop()?;
    if stack.is_empty() {
        Some(result)
    } else {
        None
    }
}

fn eval_field_test(event: DriverEvent, field: &str, value: u64) -> bool {
    match field {
        "event" => event.event_kind().code() == value,
        "processId" => event.process_id() == Some(value),
        "layer" => layer_value(event.layer()) == value,
        "tcp" | "inbound" => false,
        _ => false,
    }
}

const fn layer_mask(layer: Layer) -> LayerMask {
    match layer {
        Layer::Network => LayerMask::NETWORK,
        Layer::NetworkForward => LayerMask::NETWORK_FORWARD,
        Layer::Flow => LayerMask::FLOW,
        Layer::Socket => LayerMask::SOCKET,
        Layer::Reflect => LayerMask::REFLECT,
    }
}

const fn layer_value(layer: Layer) -> u64 {
    match layer {
        Layer::Network => 1,
        Layer::NetworkForward => 2,
        Layer::Flow => 3,
        Layer::Socket => 4,
        Layer::Reflect => 5,
    }
}
