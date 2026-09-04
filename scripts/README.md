# 可复用打包脚本

- Windows：`powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1`
- Windows 仅便携版：追加 `-SkipInstaller`
- Linux：`bash scripts/package-linux.sh`
- macOS：`bash scripts/package-macos.sh`

脚本根据 Rust 源码、Cargo 配置、Tauri 配置和前端静态资源计算 SHA-256 指纹。指纹没有变化且 `dist/<platform>` 中已有产物时，会直接复用，不再编译。需要强制重建时，Windows 追加 `-Force`，Linux/macOS 使用 `FORCE=1`。

Tauri 的安装包应在目标系统原生构建。当前实现调用了 PowerShell、netsh、pktmon 等 Windows API，所以 Windows 脚本可直接使用；Linux/macOS 脚本已具备缓存和打包流程，但要等对应后端平台适配完成后才能成功生成可用安装包。
