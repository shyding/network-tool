# network-tool

网络测试工具箱 9.0 重实现

这是对用户提供的 Windows 便携程序进行兼容性分析后建立的独立 Tauri/Rust 实现。界面使用从目标程序中依法恢复的本机前端资源，后端按原有 IPC 契约重新实现。

> 当前可运行版本已覆盖原前端识别出的 152 个业务命令入口。网络基础模块是真实实现；复杂服务类模块仍通过兼容返回保证页面可打开，详见下方状态说明。

## 从源码运行

构建产物、便携版 exe、运行时缓存和逆向截图不提交到 Git。克隆源码后可直接运行：

```powershell
cd src-tauri
cargo +1.98.0 run
```

生成 Windows 便携版：

```powershell
cd ..
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\package-windows.ps1 -SkipInstaller
```

## 验证

```powershell
cd I:\learn_code\network-toolbox-900-rebuild
python .\tests\check_ipc_coverage.py
node --experimental-vm-modules .\tests\check_protocol_view.mjs
cargo +1.98.0 check --manifest-path .\src-tauri\Cargo.toml
cargo +1.98.0 test --manifest-path .\src-tauri\Cargo.toml protocol_workbench::tests
```

预期结果是业务 IPC `152/152`，协议工作台视图检查通过，协议工作台单元测试通过。

## 实现状态

已真实实现：仪表盘快照与连通性、单次/连续/批量/TCP Ping、并发 TCP 端口扫描、路由追踪、IPv4/IPv6 子网计算、VLSM、子网拆分、路由汇总、活动路由读取与增删、网卡基础信息、本机/公网 IP 信息、MAC 格式转换、ARP 表、Wi-Fi 基础枚举、DNS/NSLookup、TCP 连通测试、连接统计与端口进程映射。

已接通兼容入口：摄像头、抓包、DHCP/LLDP、串口、远程终端、网络服务、日志/安全测试、链路监控和 AI 助手等复杂模块。这些页面不会因缺少命令而退出，但部分操作仍返回结构化兼容结果。

## 目录

- `analysis/tauri-assets/`：恢复的 108 个原版前端资源。
- `analysis/frontend-ipc.json`：按页面整理的 IPC 命令清单。
- `analysis/tauri-assets/`：当前 Tauri 前端资源。
- `src-tauri/src/`：Rust 后端实现。
- `tests/`：IPC 覆盖检查。
- `docs/`：范围、时间线和逆向报告。
