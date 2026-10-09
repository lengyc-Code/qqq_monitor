# qqq_monitor

面向 Windows 的多功能任务管理器，使用 Rust 开发。首版聚焦电脑性能监控和浅色拟态风格的原生 GUI，后续扩展进程管理、告警、服务管理等功能。

## 当前状态

已实现首个可运行版本：CPU 总体与逻辑核心、物理内存、物理磁盘读写、本地卷容量、单网卡收发，以及总览、CPU、内存、磁盘、网络、设置六个页面。数据来自真实 Windows 系统接口，界面使用原生 Rust GUI 和 DX12 渲染。

已在当前 Windows 11 非管理员账户下通过采集冒烟检查、六个页面截图检查及 8 项运行时测试。Windows 10、睡眠/热插拔、多屏 DPI、资源预算和长时间运行仍需验证；当前是开发版本，尚未完成便携发行验收。GPU、进程管理、告警与托盘功能留待后续实现。

## 设计文档

| 文档 | 内容 |
| --- | --- |
| [技术方案](docs/TECHNICAL_PLAN.md) | 首版范围、技术选型、指标口径、GUI 视觉与交互规范、性能预算和风险 |
| [架构设计](docs/ARCHITECTURE.md) | Workspace 分工、依赖方向、接口、采样与渲染数据流、异常处理和扩展步骤 |
| [实施与验收计划](docs/IMPLEMENTATION_PLAN.md) | 技术验证、开发阶段、交付顺序和可检查的验收标准 |
| [实现验证记录](docs/VALIDATION.md) | 已运行的检查、实际环境、观察结果和待验收项 |

## 已实现功能

- 性能监控：CPU 总体与逻辑核心、物理内存、本地卷容量、物理磁盘读写速率、网卡收发速率。
- 界面：浅灰拟态卡片、柔和浮雕阴影、浅蓝渐变选中效果、实时历史曲线、逻辑核心热力网格、设备选择、中文字体和可关闭的轻量动效。
- 历史：引擎保留最多 15 分钟，支持 60 秒、5 分钟、15 分钟视图；失败、断连和重新预热产生缺口。
- 设置：0.5、1、2 秒采样，默认历史范围、动效、选中网卡、窗口位置和尺寸；后台保存 TOML 配置。
- 技术栈：Rust 2024、egui/eframe、wgpu、sysinfo 和 windows。
- 模块：领域模型、Windows 采集、监控引擎、GUI；主程序负责依赖组装和启动。

磁盘 I/O 按物理磁盘统计，卷容量单独显示；网络选择一个接口，避免虚拟与物理流量重复相加。速率单位为 B/s、KiB/s、MiB/s，容量为 MiB/GiB。会话流量只累计本次应用观测到的有效计数差。

## 开发环境

使用 Windows x64、Rust 1.99 或更新版本，以及 Visual Studio Build Tools 的 C++ 工具链和 Windows SDK。构建目标为 `x86_64-pc-windows-msvc`。启动需要可用的 DX12 图形适配器；其他平台暂不支持。

在项目根目录启动：

```powershell
cargo run --release
```

也可运行 `target\release\qqq_monitor.exe`。配置由 `directories` 定位在用户目录，Windows 上通常是 `%APPDATA%\qqq_monitor\config\config.toml`；日志通常在 `%LOCALAPPDATA%\qqq_monitor\data\logs`，不写入项目目录。

## 检查与截图

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
cargo run --release -- --smoke-test

# 运行真实 GUI，约 8 秒后保存 PNG 并退出；不修改用户偏好
cargo run --release -- --capture "$PWD\target\overview.png" --page overview
```

`--smoke-test` 在约 8 秒内检查实际 CPU、内存、磁盘、网络与卷容量是否有有效结果，失败返回非零退出码。截图页面支持 `overview`、`cpu`、`memory`、`disk`、`network`、`settings`。截图路径的父目录须已存在。

## 模块分工

| 路径 | 内容 |
| --- | --- |
| `src/` | 启动、日志、CLI 检查、DX12 窗口及统一关闭 |
| `crates/qqq-domain/` | 设备与指标模型、状态、采集器和工厂接口 |
| `crates/qqq-engine/` | 独立采样调度、差分、历史、不可变视图、配置与运行时测试 |
| `crates/qqq-platform-windows/` | CPU/内存、IP Helper 网络、PDH 磁盘、卷容量与 RAII 原生调用 |
| `crates/qqq-ui/` | 六个独立页面、通用组件、主题及嵌入式中文字体 |

新增采集能力实现 `CollectorFactory` 并加入平台注册列表，引擎按统一管线处理；新增页面在 `qqq-ui/src/pages/` 实现，GUI 不依赖 Windows 采集实现。具体契约见架构文档。

## 工程约定

文件使用 UTF-8 编码。GUI 不直接采集系统数据；采集与渲染分别调度；历史数据在监控引擎中有界保存。新增功能优先沿现有模块扩展。

项目代码使用 MIT 许可证，见 [LICENSE](LICENSE)。嵌入的 Noto Sans CJK SC 字体使用 SIL Open Font License，许可见 [字体许可](crates/qqq-ui/assets/fonts/LICENSE.txt)。
