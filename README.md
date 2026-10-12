# qqq_monitor

面向 Windows 的多功能任务管理器，使用 Rust 开发。首版聚焦电脑性能监控和二次元像素风格的原生 GUI，后续扩展进程管理、告警、服务管理等功能。

## 当前状态

已实现首个可运行版本：CPU 总体与逻辑核心、物理内存、物理磁盘读写、本地卷容量、单网卡收发、GPU 利用率、引擎占用与显存，总览、CPU、内存、GPU、磁盘、网络、设置七个页面，以及可吸附在屏幕顶部或左右边缘的迷你状态栏。数据来自真实 Windows 系统接口，界面使用原生 Rust GUI 和 DX12 渲染。

已在当前 Windows 11 非管理员账户下通过采集冒烟检查（含 RTX 4090 的 GPU 读数）、七个页面与三种迷你状态栏截图检查、单屏 150% 缩放下的吸附交互检查及 13 项自动测试。Windows 10、睡眠/热插拔、多屏 DPI 与多屏吸附、集成显卡与多显卡、资源预算和长时间运行仍需验证；当前是开发版本，尚未完成便携发行验收。进程管理、告警与托盘功能留待后续实现。

## 设计文档

| 文档 | 内容 |
| --- | --- |
| [技术方案](docs/TECHNICAL_PLAN.md) | 首版范围、技术选型、指标口径、GUI 视觉与交互规范、性能预算和风险 |
| [架构设计](docs/ARCHITECTURE.md) | Workspace 分工、依赖方向、接口、采样与渲染数据流、异常处理和扩展步骤 |
| [实施与验收计划](docs/IMPLEMENTATION_PLAN.md) | 技术验证、开发阶段、交付顺序和可检查的验收标准 |
| [实现验证记录](docs/VALIDATION.md) | 已运行的检查、实际环境、观察结果和待验收项 |

## 已实现功能

- 性能监控：CPU 总体与逻辑核心、物理内存、本地卷容量、物理磁盘读写速率、网卡收发速率、GPU 利用率（按最忙引擎）、各类引擎占用、专用与共享显存。GPU 页面可切换显卡，点击引擎查看其历史。
- 边缘吸附：把窗口拖到屏幕顶部或左右边缘后松开，窗口收起为置顶的迷你状态栏，显示 CPU、内存、GPU、显存的使用率：顶部为横条，数值下方带分段条；左右两侧为 52 逻辑像素宽的竖条，四项使用率以像素仪表盘显示。GPU 与显存跟随 GPU 页面所选显卡，默认取显存最大的显卡。状态栏右下角的三个按钮可直接吸附到指定边缘。拖动迷你状态栏可沿边缘移动或换到另一条边缘；拖离边缘、双击或点击展开按钮恢复完整窗口。Windows 的贴靠与最大化手势保持系统行为。
- 界面：二次元像素风格，配色取自看板娘（青色发、粉色挑染、金色铃铛、白色上衣与黑色服装）：深色侧栏与状态栏、浅青页面、黑色描边与硬投影像素卡片、像素图标与数字字体、像素柱迷你图与 HP 式分段条、实时历史曲线、逻辑核心热力网格、全部本地卷容量一览，以及随 CPU 与内存负载切换表情的像素猫耳看板娘「Q 酱」；保留中文字体和可关闭的轻量动效。
- 历史：引擎保留最多 15 分钟，支持 60 秒、5 分钟、15 分钟视图；失败、断连和重新预热产生缺口。
- 设置：0.5、1、2 秒采样，默认历史范围、动效、边缘吸附开关、选中网卡、窗口位置和尺寸、吸附边缘与迷你状态栏位置；后台保存 TOML 配置，关闭时处于吸附状态则下次以迷你状态栏启动。
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
cargo run --release -- --capture "$PWD\target\dock-top.png" --dock top
```

`--smoke-test` 在约 8 秒内检查实际 CPU、内存、磁盘、网络与卷容量是否有有效结果，失败返回非零退出码，同时输出 GPU 读数。截图页面支持 `overview`、`cpu`、`memory`、`gpu`、`disk`、`network`、`settings`。`--dock top|left|right` 以对应边缘的迷你状态栏启动，与 `--capture` 组合时截取迷你状态栏。截图路径的父目录须已存在。

## 模块分工

| 路径 | 内容 |
| --- | --- |
| `src/` | 启动、日志、CLI 检查、DX12 窗口及统一关闭 |
| `crates/qqq-domain/` | 设备与指标模型、状态、采集器和工厂接口 |
| `crates/qqq-engine/` | 独立采样调度、差分、历史、不可变视图、配置与运行时测试 |
| `crates/qqq-platform-windows/` | CPU/内存、IP Helper 网络、PDH 磁盘与 GPU、DXGI 显卡枚举、卷容量、显示器工作区与鼠标状态查询，以及 RAII 原生调用 |
| `crates/qqq-ui/` | 七个独立页面、边缘吸附与迷你状态栏、通用组件、主题、像素绘制基础（位图字体、图标、精灵）、看板娘及嵌入式中文字体 |

新增采集能力实现 `CollectorFactory` 并加入平台注册列表，引擎按统一管线处理；新增页面在 `qqq-ui/src/pages/` 实现，GUI 不依赖 Windows 采集实现。具体契约见架构文档。

## 工程约定

文件使用 UTF-8 编码。GUI 不直接采集系统数据；采集与渲染分别调度；历史数据在监控引擎中有界保存。新增功能优先沿现有模块扩展。

项目代码使用 MIT 许可证，见 [LICENSE](LICENSE)。嵌入的 Noto Sans CJK SC 字体使用 SIL Open Font License，许可见 [字体许可](crates/qqq-ui/assets/fonts/LICENSE.txt)。
