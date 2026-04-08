# WinDivert DevBench

Task 6 wires a minimal CLI command surface and Windows host validation placeholders.

## CLI Commands

`wd-cli` currently registers these phase-one commands:

- `netdump`
- `netfilter`
- `flowtrack`
- `socketdump`
- `reflectctl`

Current behavior is intentionally minimal: each command is a placeholder entrypoint.

## Windows Host Validation Scripts

Under `tests/windows/`:

- `open_close.ps1`: smoke-runs `reflectctl`
- `network_reinject.ps1`: placeholder flow for `netfilter`
- `five_layer_observe.ps1`: placeholder flow for `netdump`, `flowtrack`, `socketdump`

Example usage:

```powershell
cargo build -p wd-cli
powershell -ExecutionPolicy Bypass -File tests/windows/open_close.ps1
```

## Driver Glue Packaging Placeholders

Under `driver/glue/`:

- `wd-rust-x64.inf`
- `wd-rust-x86.inf`
- `build.ps1` (stages INF files to `driver/glue/out`)

These are scaffolds for packaging flow documentation, not production-ready driver packages.
