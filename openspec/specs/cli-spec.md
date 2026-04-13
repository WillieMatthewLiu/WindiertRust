# WinDivert Rust CLI 规范

## 概述

`wd-cli` 是 WinDivert Rust 的命令行工具，提供 5 个子命令用于网络包捕获、过滤、重注入和事件监控。

## 命令入口

```
wd-cli <command> [options]
```

## 通用选项

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| --json | flag | false | JSON 输出模式 |
| --verbose | flag | false | 详细输出 |
| --timeout-ms | u64 | 5000 | 超时 (毫秒) |

## 子命令

### netdump

网络包捕获工具。

```
wd-cli netdump [--filter EXPR] [--count N] [--follow] [--json] [--verbose] [--timeout-ms MS]
```

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| --filter | Option\<String\> | None | 过滤表达式 (当前未实现) |
| --count | u64 | 1 | 捕获数量 |
| --follow | flag | false | 持续捕获 |

**约束**:
- `count` 必须 > 0
- `count > 1` 需要 `--follow`
- `--filter` 当前不可用 (返回 argument_error)

**层级**: Layer::Network (无 filter_ir)

**输出字段**: layer, ttl, checksum, packet_len, timestamp

### netfilter

网络过滤工具，支持三种模式。

```
wd-cli netfilter --filter EXPR [--mode MODE] [--count N] [--follow] [--json] [--verbose] [--timeout-ms MS]
```

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| --filter | String | (必填) | 过滤表达式 |
| --mode | enum | validate | 运行模式 |
| --count | u64 | 1 | 捕获数量 |
| --follow | flag | false | 持续捕获 |

**模式**:

| 模式 | 描述 |
|------|------|
| validate | 验证过滤器编译，不建立会话 |
| observe | 捕获并显示匹配的网络包 |
| reinject | 捕获一个包后重注入 |

**约束**:
- `count` 必须 > 0
- validate/reinject 模式: count 必须为 1，不允许 --follow
- observe 模式: count > 1 需要 --follow

**层级**: Layer::Network (带 filter_ir)

**validate 输出字段**: mode, layer, filter, ir_bytes

**observe 输出字段**: mode, filter, layer, ttl, checksum, packet_len, timestamp

**reinject 输出字段**: mode, filter, layer, reinjection_token, ttl, checksum, packet_len, timestamp

### flowtrack

流事件跟踪工具。

```
wd-cli flowtrack [--process-id PID] [--count N] [--follow] [--json] [--verbose] [--timeout-ms MS]
```

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| --process-id | Option\<u64\> | None | 过滤进程 ID |
| --count | u64 | 1 | 捕获数量 |
| --follow | flag | false | 持续捕获 |

**约束**:
- `count` 必须 > 0
- `count > 1` 需要 --follow

**层级**: Layer::Flow (无 filter_ir)

**输出字段**: event, flow_id, process_id, timestamp

### socketdump

Socket 事件捕获工具。

```
wd-cli socketdump --filter EXPR [--process-id PID] [--count N] [--follow] [--json] [--verbose] [--timeout-ms MS]
```

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| --filter | String | (必填) | Socket 过滤表达式 |
| --process-id | Option\<u64\> | None | 过滤进程 ID |
| --count | u64 | 1 | 捕获数量 |
| --follow | flag | false | 持续捕获 |

**约束**:
- `count` 必须 > 0
- `count > 1` 需要 --follow
- filter 必须是 Socket 层级兼容的谓词 (如 `event == CONNECT`)

**层级**: Layer::Socket (无 filter_ir，客户端过滤)

**输出字段**: event, process_id, matched, timestamp

### reflectctl

反射控制工具。

```
wd-cli reflectctl [--action ACTION] [--json] [--verbose] [--timeout-ms MS]
```

| 选项 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| --action | enum | open | 操作类型 |

**动作**:

| 动作 | 描述 |
|------|------|
| probe | 探测设备是否可用 |
| open | 打开反射句柄 |
| close | 尝试关闭反射句柄 |
| capabilities | 查询能力 |
| state | 查询状态 |

**层级**: Layer::Reflect (无 filter_ir)

**输出字段**: device, capabilities, protocol, state

---

## 运行时交互模式

所有子命令遵循统一的运行时交互模式：

1. **probe** - 探测设备可用性
2. **open** - 协商打开 (IOCTL_OPEN)
3. **open_session** - 建立会话 (IOCTL_OPEN) [仅需要 recv/send 的命令]
4. **recv/send** - 数据交换 [仅需要 recv/send 的命令]
5. **close** - 关闭会话/传输

### 超时预算

所有子命令使用 `TimeoutBudget` 机制：
- 每个运行时步骤前后检查超时
- 超时返回 `io_failure` 错误

## 错误码

| code | category | 描述 |
|------|----------|------|
| 2 | argument_error | 参数验证失败 |
| 3 | device_unavailable | 设备不可用 |
| 4 | open_failed | 打开失败 |
| 5 | protocol_mismatch | 协议版本不匹配 |
| 6 | io_failure | IO 操作失败 |

## 输出格式

### 文本模式

```
<COMMAND> OK  key1=value1  key2=value2  ...
```

### JSON 模式

```json
{"command":"<name>","status":"ok",...fields...}
```

多事件时使用 `events` 数组包裹。
