# qqq_monitor 实现验证记录

本记录区分已运行的检查与尚未完成的发行验收。当前交付可运行的 Windows 开发版本，覆盖四类真实性能监控和六个 GUI 页面；下列检查不能代替长时间运行、多机器兼容性和完整人工交互验收。

## 验证环境

| 项目 | 本次环境 |
| --- | --- |
| 系统 | Windows 11 专业版 x64，版本 10.0.26300，Build 26300 |
| 权限 | 当前进程未以管理员身份运行 |
| CPU | Intel Core i9-13900KF，24 个物理核心、32 个逻辑核心 |
| 系统可用物理内存 | 系统 API 报告约 63.76 GiB |
| 图形硬件 | NVIDIA GeForce RTX 4090，驱动 32.0.16.1714 |
| Rust | rustc/cargo 1.99，x86_64-pc-windows-msvc |
| GUI | eframe/egui 0.36.2、egui_plot 0.37.0、wgpu 30.0.1，选择 DX12 |
| 采集 | sysinfo 0.39.6、windows 0.62.2 |
| 截图 | 1920 × 1200 像素，对应默认 1280 × 800 逻辑窗口的 150% 缩放 |

图形硬件信息来自本机枚举；没有把渲染适配器当作整机 GPU 监控数据源。

## 已通过检查

| 检查 | 结果与边界 |
| --- | --- |
| cargo fmt --all -- --check | 通过 |
| cargo clippy --workspace --all-targets -- -D warnings | 通过，无警告 |
| cargo test --workspace --release | 8 项运行时测试通过，其余 crate 无机械 UI 测试 |
| cargo build --release | 成功生成 target/release/qqq_monitor.exe |
| cargo tree -d | egui 与 wgpu 各只有一套版本；部分其他传递依赖存在重复版本 |
| --smoke-test | 约 8 秒真实采集，退出码 0，核心来源均有 Ready 读数 |
| --capture 六个页面 | 总览、CPU、内存、磁盘、网络、设置均生成 PNG 后自动关闭，无启动或截图错误 |
| GUI 视觉检查 | 中文、真实设备名称、图例、曲线与核心网格可读；修复 CPU 图表横向溢出与导航缺字 |
| 配置替换与退出 | 测试验证已有文件替换、损坏文件保护，以及接受保存命令后立即关闭仍能写入 |

原生采集冒烟的一次结果：接收 25 批、丢失 0 批；CPU 和内存使用率在合法范围；两个物理磁盘都有有效读写读数，其中磁盘 1 的写入约 0.29 MiB/s；网卡有合法零值和非零收发值；三个本地卷容量有效。这些是瞬时观察，不能作为机器固定负载或性能预算。

GetIfTable2 最初会返回过滤器接口副本；当前排除 FilterInterface、EndPointInterface 和 loopback，并以 LUID 分开统计。默认选择一个已连接物理接口，当前机器选择了 USB 以太网接口；它可能没有流量，而 WLAN 有流量。用户可在网络页面切换，界面不把所有接口相加。

PDH 查询使用英文 PhysicalDisk 通配符路径和格式化数组，当前中文 Windows 的非管理员采集成功。系统缺少计数器、驱动限制或其他环境仍可能使该采集源不可用；界面保留局部失败信息。

## 浅色界面与紧凑布局验证

2026 年 10 月 5 日按参考图调整主题：统一浅灰背景、双向柔和阴影、浅蓝渐变导航和主要按钮，并同步图表网格、CPU 核心热力颜色及警告、错误文字的配色。

本次通过格式检查、Clippy 全目标检查和 Release 构建。重新运行六个页面的原生截图，每次退出码均为 0，标准错误为空；在 150% 缩放下检查了卡片、中文、指标、图例和核心网格的可读性。截图位于 target/verification。本次仅调整界面样式，未为普通 UI 拼装新增单元测试；上表的运行时测试记录来自此前的实现验证。

随后按紧凑布局要求收紧比例：侧栏宽由 188 降为 156、导航按钮高由 44 降为 36、卡片内边距由 18 降为 12；同步缩小页面标题、主指标、迷你曲线和详情图表，保留 14 像素正文。设备详情改为可换行的信息行，核心按钮高由 64 降为 50。再次通过格式检查、Clippy 和 Release 构建，并重新检查六个原生截图，退出码均为 0、标准错误为空。当前默认窗口下，总览设备概览、磁盘卷容量、网络会话流量和 32 个 CPU 核心完整可见；文字、图例与长设备标识未出现截断。此项结论仅覆盖当前窗口与本机设备，其他 DPI 和最小窗口的验收仍见下方待验收项。

## 运行时测试覆盖

| 测试文件与场景 | 实际断言 |
| --- | --- |
| normalize.rs 实际时间差与预热 | 1.2 秒增加 1200 字节得到 1000 B/s；计数回退和失败恢复重新预热 |
| normalize.rs Gauge 与合法零值 | 格式化 B/s 不再求差分；0 有效；NaN 变为失败 |
| normalize.rs 各周期过期 | 30 秒指标在第 4 秒仍有效，第 91 秒过期，成功时间不被发布时刻改写 |
| history.rs 三项限制 | 时间、1801 点和分配容量预算均生效，过期历史清理 |
| history.rs 降采样 | 峰值、时间顺序、缺口和输出点数限制保留，原始历史不被覆盖 |
| runtime_flow.rs 慢采集与显示隔离 | 用同步事件阻塞一个采集源，另一个源继续记录历史，不依赖 UI 绘制 |
| runtime_flow.rs 命令与关闭 | 32 个在途名额满后立即繁忙，消费后恢复；关闭时保存已接受配置 |
| runtime_flow.rs 配置恢复 | 按字段恢复无效项，保留其他有效项；损坏 TOML 不自动覆盖，主动保存可恢复 |

## 重现命令

在 Windows 项目根目录使用：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
cargo build --release
cargo run --release -- --smoke-test

New-Item -ItemType Directory -Path target/verification -Force | Out-Null
cargo run --release -- --capture "$PWD\target\verification\overview.png" --page overview
```

将页面参数换成 cpu、memory、disk、network、settings 可重现其他截图。截图是实际 GUI 的渲染结果，不使用假指标，不保存用户设置；父目录需存在。普通 PowerShell 调用 Windows GUI 子系统 EXE 时可能立即返回，自动检查应使用 cargo run，或 Start-Process -Wait -PassThru -WindowStyle Hidden 并重定向输出。

本轮检查输出和截图保存在被 Cargo 忽略的 target/verification，不作为另一套版本体系或项目发行物。

## 待验收和当前边界

- Windows 10、远程桌面、虚拟机、旧显卡和未安装开发工具的干净 Windows 环境尚未验证，不声明已兼容。
- 首屏时间、5 分钟整机归一化 CPU 和工作集、8 小时运行、睡眠恢复、热插拔、多屏 100%/200% DPI 与最小窗口尚未完成完整验收。
- 截图确认静态显示，未代替鼠标、键盘、滚动、设备切换及设置重启恢复的完整人工检查。
- 还需测试强制批次溢出、连续保存合并、文件只读/占用、离线目录最终清理和阻塞系统调用退出。
- 历史存储预算是包含容器容量和条目元数据的近似值，不代表整个进程的内存上限。图形资源、字体和快照额外占用内存。
- 当前日志按日滚动并最多保留 7 个文件，尚无 20 MiB 总量硬限制或采集耗时 p95 统计。
- 当前固定浅色与中文界面；GPU、进程操作、温度、告警、托盘和卷到物理盘关系未实现。
- 当前 EXE 可在本机直接运行；依赖与第三方许可汇总、便携 ZIP 和干净环境发行检查仍待完成。

继续开发优先解决稳定性和发行检查，再沿现有工厂接口与页面模块扩展功能，具体顺序见 [实施与验收计划](IMPLEMENTATION_PLAN.md)。
