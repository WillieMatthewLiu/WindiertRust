# WinDivert DevBench

`WinDivert DevBench` 是一个面向 WinDivert Rust 重写实验的 workspace。
当前仓库的重点不是提供完整可用的生产级驱动，而是把 phase-one 的协议、过滤器、用户态 API、CLI 命令面和 Windows 主机验证脚手架先打通。

如果你第一次进入仓库，建议按下面顺序理解和使用：

1. 先运行 `wd-cli`，确认命令面和本地构建正常。
2. 再执行 `tests/windows/*.ps1`，确认 Windows 主机验证脚本可跑通。
3. 如果你要做二次开发，再看各个 crate 的职责和示例代码。

## 当前状态

目前 `wd-cli` 的 5 个子命令都已经注册，但仍是 placeholder entrypoint：

- `netdump`
- `netfilter`
- `flowtrack`
- `socketdump`
- `reflectctl`

它们当前都不接收额外参数，执行后只打印占位信息并返回成功退出码。README 下文会把这些命令如何运行、如何验证、如何与脚本配合说清楚。

## 仓库结构

workspace 成员如下：

| 路径 | 作用 |
| --- | --- |
| `crates/wd-proto` | phase-one 协议对象，例如 `OpenRequest`、`OpenResponse`、`Layer`、能力位定义。 |
| `crates/wd-driver-shared` | 驱动与用户态共享常量，例如设备名和 IOCTL 常量。 |
| `crates/wd-filter` | 过滤表达式编译器与 WDIR 编解码。 |
| `crates/wd-user` | 用户态 API 骨架，负责 filter 编译、打开参数构造、事件解码与校验和修复。 |
| `crates/wd-cli` | 当前的命令行入口。 |
| `driver/wd-kmdf` | KMDF 侧占位实现与测试。 |
| `tests/windows` | Windows 主机验证脚本。 |
| `driver/glue` | INF 和打包脚本脚手架。 |

## 环境要求

当前仓库默认围绕 Windows 开发和验证：

- Rust 工具链，可执行 `cargo build` / `cargo test`
- PowerShell，用于运行 `tests/windows/*.ps1` 和 `driver/glue/build.ps1`
- 如果你只看 crate 级单元测试，不要求真的安装驱动
- 如果你要继续扩展 WinDivert/驱动联调，则需要你自行补齐签名、安装与系统权限流程；这部分当前仓库还没有完成

## 快速开始

### 1. 构建 CLI

在仓库根目录执行：

```powershell
cargo build -p wd-cli
```

构建完成后，默认可执行文件位于：

```text
.\target\debug\wd-cli.exe
```

### 2. 查看命令面

```powershell
cargo run -p wd-cli -- --help
```

当前会看到类似输出：

```text
WinDivert phase-one tooling surface

Usage: wd-cli.exe <COMMAND>

Commands:
  netdump
  netfilter
  flowtrack
  socketdump
  reflectctl
```

如果这里只看到 5 个子命令，说明当前 phase-one CLI 命令注册是正常的。

### 3. 单独运行一个命令

例如运行 `reflectctl`：

```powershell
cargo run -p wd-cli -- reflectctl
```

当前预期行为是打印：

```text
reflectctl: placeholder command surface
```

其他 4 个命令也一样，现阶段主要用于确认命令入口已经连通，而不是执行真实抓包、过滤或回注逻辑。

## CLI 使用说明

### 总体调用格式

```powershell
wd-cli.exe <subcommand>
```

当前没有额外参数，因此子命令后面不需要再跟 `--interface`、`--filter`、`--pid` 之类选项。

### `netdump`

```powershell
cargo run -p wd-cli -- netdump
```

当前用途：

- 作为 future network observe / dump 入口保留命令面
- 配合 `tests/windows/five_layer_observe.ps1` 做占位联通验证

当前不会真正输出网络数据。

### `netfilter`

```powershell
cargo run -p wd-cli -- netfilter
```

当前用途：

- 作为 future filter / reinject 入口保留命令面
- 配合 `tests/windows/network_reinject.ps1` 做占位联通验证

当前不会真正编译命令行过滤参数，也不会回注数据包。

### `flowtrack`

```powershell
cargo run -p wd-cli -- flowtrack
```

当前用途：

- 作为 future flow event 观察入口保留命令面
- 配合 `tests/windows/five_layer_observe.ps1` 做占位联通验证

当前不会输出 flow 事件。

### `socketdump`

```powershell
cargo run -p wd-cli -- socketdump
```

当前用途：

- 作为 future socket event 观察入口保留命令面
- 配合 `tests/windows/five_layer_observe.ps1` 做占位联通验证

当前不会输出 socket 事件。

### `reflectctl`

```powershell
cargo run -p wd-cli -- reflectctl
```

当前用途：

- 作为 future reflect/open-close 控制入口保留命令面
- 配合 `tests/windows/open_close.ps1` 做 smoke run

当前不会连接真实内核设备，只验证命令是否能被正常调用。

## Windows 主机验证脚本

仓库已经提供 3 个 PowerShell 脚本，目的是把 phase-one 命令面以最轻方式跑通。它们都接受一个可选参数：

```powershell
-CliPath
```

默认值是：

```text
.\target\debug\wd-cli.exe
```

这意味着你通常先在根目录执行一次：

```powershell
cargo build -p wd-cli
```

然后再运行脚本即可。

### 1. `tests/windows/open_close.ps1`

用途：对 `reflectctl` 做最小 smoke test。

```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1
```

脚本会做两件事：

1. 检查 `wd-cli.exe` 是否存在
2. 执行 `wd-cli.exe reflectctl`，并要求返回码为 0

如果你把 CLI 放在别的位置，可以显式传路径：

```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1 `
  -CliPath .\target\debug\wd-cli.exe
```

### 2. `tests/windows/network_reinject.ps1`

用途：对 `netfilter` 做占位 reinject 流程验证。

```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/network_reinject.ps1
```

当前脚本只验证：

1. CLI 文件存在
2. `wd-cli.exe netfilter` 能成功退出

它不验证真实过滤表达式下发，也不验证回注行为。

### 3. `tests/windows/five_layer_observe.ps1`

用途：对 3 个“观察类”命令做串行占位验证。

```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/five_layer_observe.ps1
```

脚本会依次运行：

1. `wd-cli.exe netdump`
2. `wd-cli.exe flowtrack`
3. `wd-cli.exe socketdump`

三者都返回 0 时，脚本才会输出 `PASS`。

## Driver Glue 打包脚手架

`driver/glue` 当前不是生产打包链路，而是占位脚手架。

目录内容：

- `driver/glue/wd-rust-x64.inf`
- `driver/glue/wd-rust-x86.inf`
- `driver/glue/build.ps1`

执行方式：

```powershell
powershell -ExecutionPolicy Bypass -File driver/glue/build.ps1
```

当前行为很简单：

1. 创建 `driver/glue/out`
2. 把两个 `.inf` 文件复制进去

这一步适合用来验证“打包阶段的文件组织”是否通顺，但它不负责：

- 驱动编译
- 驱动签名
- 驱动安装
- 生成生产可用安装包

## crate 级使用方法

如果你不是直接跑 CLI，而是要把这些 crate 集成进自己的 Rust 代码，可以先从下面几个入口开始。

### `wd-filter`：编译过滤表达式

`wd-filter` 的职责是把过滤字符串编译成内部 IR，并提供 WDIR 编解码。

最常用的入口是：

- `wd_filter::compile`
- `wd_filter::encode_ir`
- `wd_filter::decode_ir`

示例：

```rust
use wd_filter::{compile, decode_ir, encode_ir};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ir = compile("tcp and inbound and packet32[0] == 0x12345678")?;
    let bytes = encode_ir(&ir);
    let decoded = decode_ir(&bytes)?;

    assert_eq!(decoded, ir);
    Ok(())
}
```

当前源码和测试能确认的过滤表达式能力包括：

- 布尔符号：`tcp`、`inbound`
- 字段判断：`event == OPEN`、`layer == NETWORK`、`processId == 7`
- 包数据访问：`packet[0] == 1`、`packet32[0] == 0x12345678`
- 布尔组合：`and`、`or`、`not`

当前限制也要注意：

- 表达式支持范围仍然很小，不是完整 WinDivert filter 语言
- 某些 layer 不允许访问 packet 内容，例如 `FLOW` 层配合 `packet[...]` 会报错

### `wd-user`：构建用户态打开配置与处理事件

`wd-user` 当前暴露的核心类型包括：

- `HandleConfig`
- `DynamicHandle`
- `RecvEvent`
- `ChecksumUpdate`

#### 构建打开配置

```rust
use wd_user::HandleConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = HandleConfig::network("tcp and inbound")?;

    assert!(cfg.filter_ir().starts_with(b"WDIR"));
    Ok(())
}
```

这里 `HandleConfig::network` 会先调用 `wd-filter` 编译过滤器，再校验该过滤器是否与 `Layer::Network` 兼容。

#### 处理接收事件并修复校验和

```rust
use wd_user::{ChecksumUpdate, RecvEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_ipv4_frame: Vec<u8> = vec![
        0x45, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00,
        64, 0x06, 0x00, 0x00, 127, 0, 0, 1, 127, 0, 0, 1,
    ];

    let mut event = RecvEvent::decode(&raw_ipv4_frame)?;
    let change = event.packet_mut().unwrap().set_ipv4_ttl(31);
    assert_eq!(change, ChecksumUpdate::Dirty);
    event.repair_checksums()?;
    Ok(())
}
```

当前 `RecvEvent` 只覆盖 `Network` 帧场景，并要求输入至少是合法 IPv4 header。

#### 处理打开响应

```rust
use wd_proto::OpenResponse;
use wd_user::DynamicHandle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = OpenResponse::success(0x1f);
    let handle = DynamicHandle::from_open_response(response)?;

    assert_eq!(handle.capabilities_bits(), 0x1f);
    Ok(())
}
```

### `wd-proto`：协议对象

`wd-proto` 适合在驱动端与用户态之间共享 phase-one 协议定义。

当前可直接使用的对象包括：

- `ProtocolVersion`
- `Layer`
- `CapabilityFlags`
- `OpenRequest`
- `OpenResponse`

示例：

```rust
use wd_proto::{Layer, OpenRequest};

fn main() {
    let request = OpenRequest::new(Layer::Network, b"tcp and inbound".to_vec(), 0, 0);
    assert_eq!(request.filter_len as usize, request.filter_ir.len());
}
```

### `wd-driver-shared`：共享常量

如果你需要在不同模块中复用设备名与 IOCTL 编号，可以直接依赖 `wd-driver-shared`。

```rust
use wd_driver_shared::{DEVICE_NAME, IOCTL_OPEN};

fn main() {
    assert!(DEVICE_NAME.starts_with(r"\\Device\\"));
    assert_ne!(IOCTL_OPEN, 0);
}
```

## 开发者常用命令

### 构建整个 workspace

```powershell
cargo build
```

### 运行全部测试

```powershell
cargo test
```

### 只验证 CLI 命令面

```powershell
cargo test -p wd-cli
```

### 只跑过滤器相关测试

```powershell
cargo test -p wd-filter
```

### 只跑用户态 API 相关测试

```powershell
cargo test -p wd-user
```

## 当前限制与预期边界

这一节很重要，因为当前仓库很容易被误解成“已经完成 WinDivert 替代实现”。

实际情况是：

- CLI 已经有命令入口，但都还是 placeholder
- Windows 验证脚本当前只做“命令存在并可返回 0”的 smoke / scaffold 验证
- `driver/glue` 只是 INF 和 staging 脚手架，不是完整驱动发布流
- 过滤器语言只覆盖少量字段、符号和值
- `wd-user` 当前只覆盖很小一部分用户态协议与事件处理流程
- KMDF 目录已有测试和骨架，但不等同于生产可部署驱动

如果你接下来要继续推进，比较合理的顺序通常是：

1. 先把某一个 CLI 子命令从 placeholder 变成真正的参数化命令
2. 再让对应的 PowerShell 验证脚本从“只看退出码”升级到“验证真实行为”
3. 最后再补齐打包、签名、安装和端到端联调文档
