# WinDivert Rust 过滤器规范

## 概述

WinDivert Rust 实现了一个类 BPF 的过滤器编译器，将文本过滤表达式编译为二进制 IR (Intermediate Representation)，供内核态 FilterEngine 执行。

## 编译管线

```
文本输入 -> Lexer -> Token 流 -> Parser -> AST -> Semantics -> FilterIr -> 二进制 WDIR
```

## 词法分析 (Lexer)

### Token 类型

| Token | 描述 |
|-------|------|
| Ident(String) | 标识符 |
| Number(u64) | 数字字面量 |
| EqEq | `==` |
| LBracket | `[` |
| RBracket | `]` |
| LParen | `(` |
| RParen | `)` |
| And | `and` |
| Or | `or` |
| Not | `not` |

### 字面量

- 十进制整数: `0..=18446744073709551615`
- 十六进制整数: `0x` 前缀
- IPv4 地址: `1.2.3.4`
- IPv4 CIDR: `1.2.3.0/24`

## 语法 (Parser)

### 优先级 (从低到高)

1. `or` - 逻辑或
2. `and` - 逻辑与
3. `not` - 逻辑非
4. 原子表达式

### AST

```rust
enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Predicate(Predicate),
}

enum Predicate {
    BareSymbol(String),                    // e.g. "tcp"
    FieldEq { field: String, value: Value }, // e.g. "processId == 42"
    PacketEq { width, offset, value },     // e.g. "packet[0] == 0x45"
}
```

### 谓词形式

1. **裸符号**: `tcp`, `udp`, `ipv4`, `ipv6`, `outbound`, `inbound`
2. **字段等值**: `field == value`
3. **包偏移等值**: `packet[offset] == value`, `packet16[offset] == value`, `packet32[offset] == value`

## 语义分析 (Semantics)

### 层级掩码推导

过滤器编译器根据谓词自动推导所需的层级：

| 谓词 | 推导的层级 |
|------|-----------|
| `tcp`, `udp` | (不自动推导层级，需配合其他谓词) |
| `ipv4`, `ipv6` (正极性) | NETWORK + NETWORK_FORWARD |
| `outbound` (正极性) | NETWORK_FORWARD |
| `inbound` (正极性) | NETWORK |
| `event == open` (正极性) | REFLECT |
| `event == connect` (正极性) | SOCKET |
| `event == close` (正极性) | REFLECT |
| `layer == flow` (正极性) | FLOW |
| `layer == network` (正极性) | NETWORK |
| `layer == network_forward` (正极性) | NETWORK_FORWARD |
| `layer == socket` (正极性) | SOCKET |
| `layer == reflect` (正极性) | REFLECT |
| `processId == N` (正极性) | SOCKET |
| `localPort`, `remotePort`, `localAddr`, `remoteAddr` (正极性) | NETWORK + NETWORK_FORWARD |
| `packet[N] == V` | NETWORK (needs_payload=true) |

### 约束

- `packet` 访问与 FLOW 层级不兼容
- `event` 字段期望符号值，不接受数字
- `layer` 字段期望符号值，不接受数字
- `processId` 字段期望数字值，不接受符号

## 字段映射

| 源文本 | 规范字段名 |
|--------|-----------|
| event | event |
| layer | layer |
| processid | processId |
| tcp | tcp |
| udp | udp |
| ipv4 | ipv4 |
| ipv6 | ipv6 |
| localaddr | localAddr |
| remoteaddr | remoteAddr |
| localport | localPort |
| remoteport | remotePort |
| outbound | outbound |
| inbound | inbound |

## 值映射

### event 符号值

| 符号 | 数值 |
|------|------|
| open | 1 |
| connect | 2 |
| close | 3 |
| established | 4 |

### layer 符号值

| 符号 | 数值 |
|------|------|
| network | 1 |
| network_forward | 2 |
| flow | 3 |
| socket | 4 |
| reflect | 5 |

### IPv4 地址编码

IPv4 地址编码为 u64：
- 高 8 位: CIDR 前缀长度
- 低 32 位: 网络地址 (已与掩码做 AND)

```
encoded = (prefix as u64 << 32) | (network_addr as u64)
```

## IR 二进制格式 (WDIR)

### 头部

| 字段 | 偏移 | 长度 | 描述 |
|------|------|------|------|
| magic | 0 | 4 | "WDIR" |
| version | 4 | 1 | 版本号 = 1 |
| required_layers | 5 | 1 | 层级掩码 (u8) |
| needs_payload | 6 | 1 | 是否需要载荷 (0/1) |

### 引用字段段

| 字段 | 长度 | 描述 |
|------|------|------|
| field_count | 2 | 字段数量 (u16 LE) |
| fields... | 变长 | 每个字段: len(u16 LE) + bytes |

### 程序段

| 字段 | 长度 | 描述 |
|------|------|------|
| program_len | 4 | 指令数量 (u32 LE) |
| opcodes... | 变长 | 指令序列 |

### OpCode 编码

| OpCode | ID | 参数 | 描述 |
|--------|-----|------|------|
| FieldTest | 1 | field_len(u16) + field_bytes + value(u64) | 字段测试 |
| PacketLoad32 | 2 | offset(u16) + value(u32) | 32位包偏移比较 |
| PacketLoad8 | 3 | offset(u16) + value(u8) | 8位包偏移比较 |
| And | 4 | (无) | 逻辑与 |
| Or | 5 | (无) | 逻辑或 |
| Not | 6 | (无) | 逻辑非 |
| PacketLoad16 | 7 | offset(u16) + value(u16) | 16位包偏移比较 |

### 限制

| 参数 | 最大值 |
|------|--------|
| referenced_fields | 256 |
| program_len | 4096 |
| field_byte_len | 32 |

## LayerMask 位定义

| 位 | 层级 |
|-----|------|
| 0 (0x01) | NETWORK |
| 1 (0x02) | NETWORK_FORWARD |
| 2 (0x04) | FLOW |
| 3 (0x08) | SOCKET |
| 4 (0x10) | REFLECT |

有效位掩码: `0b0001_1111`

## FilterEngine 运行时验证

### 层级与字段兼容性

| 层级 | 允许的字段 |
|------|-----------|
| Network / NetworkForward | packet, tcp, udp, ipv4, ipv6, localAddr, remoteAddr, localPort, remotePort, inbound, outbound, layer |
| Socket | event (仅 Connect), processId, layer |
| Flow | event (仅 Established), processId, layer |
| Reflect | event (仅 Open/Close), layer |

### 程序形状验证

- 程序不能为空
- 使用栈深度跟踪验证：FieldTest/PacketLoad 压栈，And/Or 弹 2 压 1，Not 弹 1 压 1
- 最终栈深度必须为 1

### 包载荷谓词

- PacketLoad 指令仅在 Network / NetworkForward 层级允许
- 包偏移读取使用大端序 (network byte order)
