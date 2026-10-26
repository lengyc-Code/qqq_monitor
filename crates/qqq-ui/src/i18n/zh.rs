//! Simplified Chinese.

use super::{PageText, Strings};
use qqq_domain::{Reason, Shortcut};
use qqq_engine::{Notice, SaveStatus};

pub const STRINGS: Strings = Strings {
    window_title: "QQQ Monitor · 性能监控",
    tagline: "性能监控",
    startup_failed: "QQQ Monitor 启动失败",
    overview_page: PageText {
        nav: "总览",
        title: "系统总览",
        hint: "实时掌握电脑的运行状态",
    },
    cpu_page: PageText {
        nav: "CPU",
        title: "处理器",
        hint: "观察整体负载与每个逻辑核心",
    },
    memory_page: PageText {
        nav: "内存",
        title: "物理内存",
        hint: "系统可用物理内存与使用趋势",
    },
    gpu_page: PageText {
        nav: "GPU",
        title: "显卡",
        hint: "显卡利用率、各引擎占用与显存",
    },
    frames_page: PageText {
        nav: "帧率",
        title: "帧率与帧时间",
        hint: "游戏时观察前台应用的流畅度",
    },
    disk_page: PageText {
        nav: "磁盘",
        title: "磁盘与存储",
        hint: "物理磁盘读写与本地卷容量",
    },
    network_page: PageText {
        nav: "网络",
        title: "网络流量",
        hint: "查看所选网卡的实时收发",
    },
    processes_page: PageText {
        nav: "进程",
        title: "进程",
        hint: "查看并管理 Windows 与运行中的 WSL 发行版里的进程",
    },
    remote_page: PageText {
        nav: "远程",
        title: "远程主机",
        hint: "通过 SSH 监控 Linux 主机的系统信息与主要指标",
    },
    alerts_page: PageText {
        nav: "告警",
        title: "监控告警",
        hint: "指标越过阈值时推送 Windows 通知",
    },
    settings_page: PageText {
        nav: "设置",
        title: "设置",
        hint: "按你的使用习惯调整监控",
    },

    history_range: "历史范围",
    seconds: |v| format!("{v} 秒"),
    minutes: |v| format!("{v} 分钟"),
    running: "监控运行中",
    degraded: "部分采集不可用",
    timing: |sampling, session| format!("采样 {sampling:.1}s  ·  会话 {session:.0}s"),
    minibar: "迷你状态栏",
    alerts_firing: |n| format!("{n} 条告警触发中"),
    dock_top: "吸附到屏幕顶部",
    dock_left: "吸附到屏幕左侧",
    dock_right: "吸附到屏幕右侧",
    restore_window: "还原完整窗口",

    tray_open: "显示主窗口",
    tray_exit: "退出",

    days: |v| format!("{v} 天"),
    state_ready: "实时",
    state_stale: "数据过期",
    state_failed: "采集失败",
    state_unsupported: "不支持",
    state_offline: "已断开",
    state_waiting: "采集中",
    seconds_ago: |v| format!("{v:.0} 秒前"),
    minutes_ago: |v| format!("{v} 分钟前"),
    last_valid: "上次有效数据",
    now: "现在",
    chart_waiting: "等待有效采样；数据不可用时曲线保留缺口",
    device_offline: "设备已断开",
    offline_suffix: "（已断开）",
    finding_devices: "等待发现设备",
    waiting_samples: "等待采样",
    click_for_details: "点击查看详情",
    list_separator: "，",
    labeled: |label, text| format!("{label}：{text}"),
    used_of: |used, total| format!("已用 {used} / {total}"),

    processor: "处理器",
    physical_memory: "物理内存",
    this_pc: "本机",
    physical_disk: "物理磁盘",
    graphics_card: "显卡",
    foreground_app: "前台应用",
    adapter: "网卡",
    logical_core: |n| format!("逻辑核心 {n}"),
    detail,
    physical_adapter: "物理网卡",
    virtual_adapter: "虚拟或软件接口",
    dedicated_memory: "专用显存",
    shared_memory: "共享 GPU 内存",
    reason,
    notice,
    save_status,

    memory: "内存",
    gpu_memory: "显存",
    memory_short: "内存",
    gpu_memory_short: "显存",
    read: "读取",
    write: "写入",
    receive: "接收",
    send: "发送",
    logical_cores: "逻辑核心",
    metric,

    mint_name: "看板娘 · MINT 酱",
    mint_hover: "看板娘 MINT 酱会根据 CPU 与内存负载切换表情",
    mint_waiting: "正在唤醒传感器…\n请稍等一下～",
    mint_trouble: "有些数据采集不到…\n去「设置」看看吧",
    mint_cpu_hot: |cpu| format!("CPU {cpu:.0}%！\n忙得要冒烟啦 >_<"),
    mint_memory_full: |memory| format!("内存用了 {memory:.0}%\n快要装不下啦！"),
    mint_busy: |cpu, memory| format!("有点忙哦～\nCPU {cpu:.0}%  内存 {memory:.0}%"),
    mint_relaxed: |cpu| format!("好悠闲～CPU 才 {cpu:.0}%\n一起喝杯茶吧"),
    mint_calm: |cpu, memory| format!("一切正常！\nCPU {cpu:.0}%  内存 {memory:.0}%"),

    cores_detail: |cores| format!("逻辑核心 {cores}"),
    disk_read: "磁盘读取",
    network_receive: "网络接收",
    cpu_memory_chart: "处理器与内存使用趋势",
    disk_io: "磁盘读写",
    network_io: "网络收发",
    device_profile: "设备档案",
    total_memory: "内存总量",
    current_disk: "当前磁盘",
    current_adapter: "当前网卡",
    uptime: "系统运行",

    cpu_total: "总使用率",
    cpu_total_detail: "所有逻辑核心的平均负载",
    cpu_busiest: "最忙核心",
    cpu_overall: "整体 CPU",
    cpu_selected_core: "选中核心",
    cpu_chart: "CPU 使用率",
    cpu_cores_hint: "点击核心查看历史",

    memory_in_use: "已使用",
    share_of_total: |share| format!("占总量 {share}"),
    available: "可用",
    allocatable: |share| format!("还能分配 {share}"),
    memory_total: "物理内存总量",
    memory_total_detail: "系统可用的物理内存",
    memory_chart: "物理内存使用趋势",
    memory_usage: "内存使用率",
    memory_note: "统计系统可用物理内存；已用内存与提交量属于不同指标。",

    gpu_usage: "GPU 使用率",
    engines_waiting: "等待引擎数据",
    busiest_engine: |engine, load| format!("最忙引擎 {engine} · {load}"),
    gpu_memory_used: |kind, used, total| format!("{kind} 已用 {used} / {total}"),
    gpu_chart: "GPU 与显存使用率",
    engines_title: "引擎占用",
    engines_hint: "同类引擎取最忙的一个，点击查看历史",
    engines_missing: "等待引擎数据；未提供引擎计数器的显卡只显示显存",
    engine: |name| format!("{name} 引擎"),
    engine_kind,
    gpu_memory_title: "显存明细",
    gpu_memory_hint: "专用显存位于显卡板载内存，共享 GPU 内存取自系统内存",
    total_share: |total, share| format!("共 {total}  ·  占 {share}"),
    gpu_note: "GPU 使用率与任务管理器口径一致：各进程在同一引擎上的占用相加，再取最忙的引擎；不同类型的引擎不相加。",

    disk_read_detail: "选中物理磁盘的读取速率",
    disk_write_detail: "选中物理磁盘的写入速率",
    disk_chart: "物理磁盘读写速率",
    volumes_title: "本地卷容量",
    volumes_hint: "卷容量与物理磁盘 I/O 分别统计",
    volumes_waiting: "等待发现本地卷",
    volume_space: |free, total| format!("可用 {free}  ·  共 {total}"),
    no_label: "无卷标",
    unknown_file_system: "未知文件系统",

    session_received: |bytes| format!("本次会话已接收 {bytes}"),
    session_sent: |bytes| format!("本次会话已发送 {bytes}"),
    network_chart: "网络收发趋势",
    network_note: "只统计选中的单个接口，避免物理网卡与 VPN 流量重复相加；会话流量为本次应用观测到的有效计数差。",

    preferences: "监控偏好",
    language: "界面语言",
    follow_system: "跟随系统",
    sampling_interval: "采样间隔",
    default_range: "默认历史范围",
    animations: "界面动效",
    animations_option: "启用轻量动效与看板娘晃动",
    edge_docking: "边缘吸附",
    edge_docking_option: "拖到屏幕顶部或左右边缘时收起为迷你状态栏",
    minimize_to_tray: "最小化到托盘",
    minimize_to_tray_option: "最小化时只在任务栏托盘保留图标",
    autostart: "开机自动启动",
    autostart_option: "登录 Windows 时自动启动 QQQ Monitor",
    autostart_to_tray_option: "开机启动时直接收进托盘",
    frame_monitoring: "帧率监控",
    frame_monitoring_option: "统计前台应用的帧率与帧时间，迷你状态栏同时显示 FPS",
    apply_save: "应用并保存",
    runtime_status: "运行状态",
    batches: |received, dropped| format!("接收采样批次 {received}  ·  丢失批次 {dropped}"),
    history_usage: |used| format!("历史数据占用 {used} / 16 MiB"),
    collectors_ok: "所有采集源工作正常",
    refresh_devices: "立即刷新设备",
    about: "关于",
    about_intro: "二次元像素风格的 Windows 多功能任务管理器，由看板娘「MINT 酱」陪你看着电脑的状态：实时性能监控与历史曲线、进程管理、通过 SSH 监控 Linux 主机、前台应用的帧率统计、监控告警，以及截图、剪贴板管理与便签等小工具。",
    about_github: "GitHub 主页",

    process_name: "名称",
    process_user: "用户",
    process_memory: "内存",
    process_threads: "线程",
    process_priority: "优先级",
    process_command: "命令行",
    process_search: "搜索名称、PID、用户或命令行",
    process_count: |shown, total| {
        if shown == total {
            format!("共 {total} 个进程")
        } else {
            format!("显示 {shown} / {total} 个进程")
        }
    },
    processes_loading: "正在读取进程…",
    processes_none: "没有匹配的进程",
    processes_failed: |reason| format!("无法读取进程：{reason}"),
    wsl_hint: "WSL 发行版运行后会显示在这里",
    end_process: "结束进程",
    kill_process: "强制结束",
    end_confirm: |name, pid| format!("结束进程“{name}”（PID {pid}）？"),
    end_warning: "进程中未保存的数据会丢失。",
    kill_warning: "强制结束会立即终止进程，未保存的数据会丢失。",
    cancel: "取消",
    priorities: ["低", "低于正常", "正常", "高于正常", "高", "实时"],

    remote_add: "添加主机",
    remote_empty: "还没有远程主机。添加一台能用 SSH 登录的 Linux 主机，即可在这里查看它的系统信息、CPU、内存、磁盘、网络与文件系统。",
    remote_connecting: "正在连接…",
    remote_connected: "已连接",
    remote_failed: |reason| format!("连接失败：{reason}"),
    remote_retrying: "稍后自动重试",
    remote_key_unknown: |fingerprint| {
        format!("首次连接这台主机。请向服务器管理员核对主机密钥指纹，一致后再信任：\n{fingerprint}")
    },
    remote_key_changed: |fingerprint| {
        format!(
            "主机密钥与之前信任的不同：可能是服务器重装了系统，也可能有人在冒充它。确认无误后再信任新密钥：\n{fingerprint}"
        )
    },
    remote_trust: "信任并连接",
    remote_trust_new: "信任新密钥",
    remote_edit: "编辑",
    remote_reconnect: "重新连接",
    remote_remove: "删除",
    remote_remove_confirm: |name| format!("删除远程主机“{name}”？"),
    remote_remove_warning: "凭据管理器中保存的密码或口令以及已采集的数据会一并删除。",
    remote_auth_password: "密码",
    remote_auth_key: "私钥文件",
    remote_auth_agent: "SSH 代理",
    remote_add_title: "添加远程主机",
    remote_edit_title: "编辑远程主机",
    remote_import: "从 SSH 配置导入",
    remote_name: "名称",
    remote_name_hint: "可选，留空时显示地址",
    remote_address: "地址",
    remote_address_hint: "主机名或 IP 地址",
    remote_port: "端口",
    remote_user: "用户名",
    remote_auth: "登录方式",
    remote_password: "密码",
    remote_password_keep: "留空则保留已保存的密码",
    remote_key_file: "私钥文件",
    remote_key_hint: "例如 ~/.ssh/id_ed25519",
    remote_passphrase: "私钥口令",
    remote_passphrase_hint: "私钥未加密时留空",
    remote_passphrase_keep: "留空则保留已保存的口令",
    remote_agent_hint: "使用 Windows OpenSSH 认证代理服务或 Pageant 中已加载的密钥。",
    remote_secret_note: "密码与私钥口令保存在 Windows 凭据管理器中，不写入配置文件。",
    remote_save: "保存并连接",
    remote_missing: |field| format!("请填写{field}"),
    remote_bad_port: "端口应为 1 至 65535 之间的数字",
    remote_hostname: "主机名",
    remote_load: "平均负载",
    remote_cpu_detail: |iowait, processes| format!("I/O 等待 {iowait} · {processes} 个进程"),
    remote_swap: "交换空间",
    remote_no_swap: "未启用",
    remote_volumes_title: "文件系统",
    remote_volumes_hint: "本地文件系统的容量，使用率与 df 的口径一致",
    remote_cores_hint: "每个逻辑核心的当前负载",
    yes: "是",

    fps: "帧率",
    frame_time: "帧时间",
    frame_time_max: "最长帧时间",
    fps_detail: |mean| format!("平均帧时间 {mean}"),
    frame_time_detail: |longest| format!("最长 {longest}"),
    frames_target: "统计对象",
    frames_off: "帧率监控已关闭。开启后在后台统计前台应用每秒呈现的画面数与相邻两帧的间隔，迷你状态栏同时显示 FPS。",
    frames_denied: "读取系统的图形事件需要管理员权限。可以管理员身份重新启动，或把当前账户加入 Performance Log Users 组后重新登录 Windows。",
    frames_restart: "以管理员身份重新启动",
    frames_waiting: "切换到游戏或其他应用后，开始统计它的画面",
    fps_chart: "帧率趋势",
    frame_time_chart: "帧时间趋势",
    frames_note: "帧率为前台应用每秒呈现的画面数，帧时间为相邻两次呈现的间隔；数据来自 Windows 对 DXGI 与 Direct3D 9 的事件跟踪，不介入游戏进程。切换到本程序时继续统计上一个应用；不经 DXGI 呈现的 Vulkan 与 OpenGL 程序可能没有数据。",

    alert_add: "添加告警",
    alert_test: "发送测试通知",
    alert_test_body: "收到这条通知，说明告警可以正常推送。",
    alert_empty: "还没有告警规则。选择设备与指标并设置阈值后，读数越过阈值时会推送 Windows 通知。",
    alert_rules: "告警规则",
    alert_recent: "最近告警",
    alert_recent_empty: "本次运行还没有触发告警",
    alert_normal: "正常",
    alert_no_data: "等待数据",
    alert_disabled: "已停用",
    alert_pending: |left| format!("已越过阈值，{left}后告警"),
    alert_firing: |held| format!("告警中 · 已持续 {held}"),
    alert_now: |value| format!("当前 {value}"),
    alert_above: "高于",
    alert_below: "低于",
    alert_condition: |above, threshold, hold| {
        let side = if above { "高于" } else { "低于" };
        match hold {
            Some(hold) => format!("{side} {threshold} · 持续 {hold}"),
            None => format!("{side} {threshold}"),
        }
    },
    alert_hold_none: "立即",
    alert_add_title: "添加告警规则",
    alert_edit_title: "编辑告警规则",
    alert_device: "设备",
    alert_metric: "指标",
    alert_when: "条件",
    alert_threshold: "阈值",
    alert_hold: "持续时间",
    alert_enabled: "启用此规则",
    alert_pick_device: "选择设备",
    alert_device_missing: "设备暂未连接",
    alert_missing: "请选择设备与指标",
    alert_bad_threshold: "阈值应为数字",
    alert_save: "保存",
    alert_edit: "编辑",
    alert_remove: "删除",
    alert_title: |device, metric, above| {
        format!("{device} {metric}{}", if above { "过高" } else { "过低" })
    },
    alert_body: |value, threshold, above, hold| {
        let side = if above { "高于" } else { "低于" };
        match hold {
            Some(hold) => format!("当前 {value}，{side}阈值 {threshold}，已持续 {hold}"),
            None => format!("当前 {value}，{side}阈值 {threshold}"),
        }
    },
    alert_note: "规则读取各采集源的最新有效读数：读数越过阈值并保持设定的持续时间后推送 Windows 通知，回到另一侧并保持同样时间后恢复正常；同一规则两次通知至少间隔 1 分钟。窗口收进托盘或吸附为迷你状态栏时同样推送，点击通知可打开主窗口。",

    tools_page: PageText {
        nav: "小工具",
        title: "小工具",
        hint: "截图、剪贴板管理、便签与快捷指令等实用工具",
    },
    tool_screenshots: "截图",
    tool_clipboard: "剪贴板",
    tool_notes: "便签",
    tool_commands: "快捷指令",
    tray_screenshot: "截图",
    hours_ago: |v| format!("{v} 小时前"),
    shot_take: "开始截图",
    shot_hint: "拖动选择截图区域 · 单击截取整个屏幕 · 右键或 Esc 取消",
    shot_folder: "打开文件夹",
    shot_hotkey: "快捷键",
    shot_hotkey_option: |key| format!("按 {key} 截图"),
    shot_hotkey_change: "修改",
    shot_hotkey_press: "请按下新的快捷键，Esc 取消",
    shot_hotkey_invalid: "快捷键需要 Ctrl 或 Alt 加字母或数字，也可以是 F1 至 F24",
    keep: "保留数量",
    shot_count: |n| format!("{n} 张"),
    shot_recent: "最近的截图",
    shot_empty: "还没有截图。按下快捷键或点击「开始截图」。",
    shot_loading: "正在读取截图…",
    shot_busy: "截图存储正忙，请在待保存的截图写入后重试",
    shot_memory_full: "截图图片内存已满，请在处理中的截图完成后重试",
    shot_too_large: "截图图片过大",
    shot_storage_unavailable: "截图存储不可用",
    shot_note: |folder| {
        format!(
            "松开鼠标后，所选区域复制到剪贴板并以 PNG 保存在 {folder}；超出保留数量时先删除最早的截图。点击「开始截图」时，主窗口先隐藏再截取。"
        )
    },
    copy: "复制",
    open: "打开",
    delete: "删除",

    clip_option: "记录复制到剪贴板的文本、图片与文件",
    clip_off: "剪贴板管理未开启。开启后会记录此后复制的内容。",
    clip_clear: "清空记录",
    clip_count: |n| format!("{n} 条"),
    clip_recent: "剪贴板记录",
    clip_empty: "还没有记录。复制文本、图片或文件后会显示在这里。",
    clip_text: |n| format!("文本 · {n} 字"),
    clip_picture: |w, h| format!("图片 · {w} × {h}"),
    clip_files: |n| format!("{n} 个文件"),
    clip_more: |n| format!("另有 {n} 项"),
    clip_note: "点击记录即可复制回剪贴板，最新的记录排在最前。记录只保存在内存中，退出程序后清空；密码管理器标记为不记录的内容会被跳过；图片合计最多占用 256 MiB 内存。",

    tray_note: "便签",
    note_title: "QQQ 便签",
    note_hotkey_option: |key| format!("按 {key} 显示或隐藏便签"),
    hotkey_taken: "这个组合键已用于另一个快捷键",
    hotkey_none: "未设置快捷键",
    hotkey_set: "设置",
    note_open: "打开便签",
    note_new: "新建便签",
    note_list: "全部便签",
    note_write: "编辑",
    note_read: "预览",
    note_delete: "删除便签",
    note_hide: "隐藏便签",
    note_untitled: "无标题便签",
    note_hint: "在这里记录，支持 Markdown",
    note_read_empty: "这条便签还是空的。双击此处或按 Ctrl+E 开始书写。",
    note_delete_confirm: "删除这条便签？",
    note_delete_confirm_short: "确认删除",
    note_chars: |n| format!("{n} 字"),
    note_saving: "正在保存…",
    note_saved: "已保存",
    note_save_failed: |error| format!("保存失败：{error}"),
    note_storage_unavailable: "便签存储不可用",
    note_count: |n| format!("{n} 条"),
    note_empty: "还没有便签。按下快捷键或点击「打开便签」开始记录。",
    note_loading: "正在读取便签…",
    note_about: |folder| {
        format!(
            "便签窗口始终位于其他窗口之上：按快捷键显示，在便签中再按一次隐藏；拖动标题栏移动，拖动右下角调整大小。内容使用 Markdown，按 Ctrl+E 在编辑与预览之间切换，按 Ctrl+N 新建便签；预览中可以勾选任务、打开网页链接。每条便签是 {folder} 中的一个 .md 文件，停止输入片刻后自动保存。"
        )
    },

    command_hotkey_option: |key| format!("按 {key} 运行"),
    command_new: "新建快捷指令",
    command_list: "全部快捷指令",
    command_count: |n| format!("{n} 条"),
    command_empty: "还没有快捷指令。点击「新建快捷指令」，填写要运行的 PowerShell 命令。",
    command_run: "运行",
    command_stop: "终止",
    command_running: |n| format!("运行中 · {n} 个会话"),
    command_delete_ends: |n| format!("删除会同时结束它在后台运行的 {n} 个会话"),
    command_sound_option: "运行快捷指令时播放提示音",
    command_balloon: |n| format!("已开始运行，共 {n} 个会话"),
    command_edit: "编辑",
    command_add_title: "新建快捷指令",
    command_edit_title: "编辑快捷指令",
    command_name: "名称",
    command_lines: "命令",
    command_lines_hint: "每行一条 PowerShell 命令",
    command_sessions: "会话",
    command_shared: "在同一会话中依次运行",
    command_separate: "每条命令一个会话",
    command_window: "窗口",
    command_window_option: "显示 PowerShell 窗口，运行结束后保留",
    command_directory: "工作目录",
    command_directory_hint: "留空则从用户文件夹开始",
    command_save: "保存",
    command_missing_name: "请填写名称",
    command_missing_lines: "请至少填写一条命令",
    command_too_long: |lines, bytes| {
        format!("命令过多或过长：最多 {lines} 行，合计约 {bytes} 个英文字符")
    },
    command_bad_directory: "找不到这个文件夹，请填写完整路径",
    command_summary: |shared, lines, window| {
        format!(
            "{} · {lines} 条命令 · {}",
            if shared {
                "同一会话"
            } else {
                "每条命令一个会话"
            },
            if window {
                "显示窗口"
            } else {
                "后台运行"
            }
        )
    },
    command_about: "快捷指令用 Windows PowerShell 运行。「在同一会话中依次运行」时，全部命令在一个会话中按顺序执行，后面的命令沿用前面设置的变量与当前位置；「每条命令一个会话」时，每条命令同时在各自的会话中启动。显示窗口时，命令结束后窗口保留，便于查看输出或继续输入；不显示窗口时命令在后台运行，结束后会话随之退出。会话运行期间，快捷指令显示运行中的会话数与「终止」按钮，点击后结束它的全部会话以及会话中启动的程序。每次运行播放提示音。快捷键是全局快捷键，窗口收进托盘或吸附为迷你状态栏时同样可用，此时还会在托盘弹出气泡显示运行结果；点击快捷指令中的「设置」后按下组合键即可指定。",
};

fn detail(key: &str) -> Option<&'static str> {
    Some(match key {
        "logical_cores" => "逻辑核心",
        "os" => "操作系统",
        "description" => "描述",
        "interface_guid" => "接口 GUID",
        "interface_type" => "接口类型",
        "link_speed" => "链路速率",
        "vendor" => "厂商",
        "memory_type" => "显存类型",
        "adapter_luid" => "适配器 LUID",
        "counter_instance" => "计数器实例",
        "volume_label" => "卷标",
        "file_system" => "文件系统",
        "volume_guid" => "卷 GUID",
        "kernel" => "内核",
        "architecture" => "架构",
        "default_route" => "默认路由",
        "device" => "设备",
        "process_id" => "进程 ID",
        "present_api" => "呈现接口",
        _ => return None,
    })
}

fn metric(name: &str) -> Option<&'static str> {
    Some(match name {
        "usage" => "使用率",
        "iowait" => "I/O 等待",
        "used" => "已用",
        "available" => "可用",
        "swap_usage" => "交换空间使用率",
        "memory_usage" => "显存使用率",
        "memory_used" => "显存已用",
        "read" => "读取速率",
        "write" => "写入速率",
        "receive" => "接收速率",
        "send" => "发送速率",
        "load1" => "1 分钟平均负载",
        "load5" => "5 分钟平均负载",
        "load15" => "15 分钟平均负载",
        "processes" => "进程数",
        "fps" => "帧率",
        "frame_time" => "平均帧时间",
        "frame_time_max" => "最长帧时间",
        _ => return None,
    })
}

fn engine_kind(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "copy" => "复制",
        "videodecode" => "视频解码",
        "videoencode" => "视频编码",
        "videoprocessing" => "视频处理",
        "compute" => "计算",
        "graphics" => "图形",
        "legacyoverlay" => "旧版叠加",
        "overlay" => "叠加",
        "security" => "安全",
        "ofa" => "光流",
        _ => return None,
    })
}

fn reason(reason: &Reason) -> String {
    match reason {
        Reason::NoGpuCounters => "系统未提供该显卡的性能计数器".into(),
        Reason::NoGpuMemoryCounters => "显存计数器不可用".into(),
        Reason::VolumeUnavailable => "卷容量暂时不可用".into(),
        Reason::NoMemoryTotal => "物理内存总量不可用".into(),
        Reason::OutOfRange => "采集值超出有效范围".into(),
        Reason::CounterUnit => "计数器单位无效".into(),
        Reason::Disconnected => "设备已断开".into(),
        Reason::Panicked => "采集线程发生异常，正在重建采集器".into(),
        Reason::KeyConflict => "指标键与其他采集器冲突".into(),
        Reason::InstancesChanging => "计数器实例变化频繁，暂时无法读取".into(),
        Reason::CallFailed { api, code } => format!("{api} 失败，错误码 0x{code:08X}"),
        Reason::BadData { api } => format!("{api} 返回的数据无效"),
        Reason::ProcessGone => "进程已经退出".into(),
        Reason::AccessDenied => "没有权限，可以管理员身份运行后重试".into(),
        Reason::HostStopped => "WSL 发行版已停止".into(),
        Reason::AddressNotFound => "找不到该主机名".into(),
        Reason::ConnectionRefused => "连接被拒绝，请检查地址、端口和 SSH 服务".into(),
        Reason::TimedOut => "连接超时".into(),
        Reason::ConnectionLost => "连接已断开".into(),
        Reason::Handshake => "SSH 握手失败，双方没有共同支持的算法".into(),
        Reason::HostKeyUnknown { .. } => "主机密钥尚未受信任".into(),
        Reason::HostKeyChanged { .. } => "主机密钥与之前信任的不同".into(),
        Reason::AuthFailed => "用户名、密码或密钥被拒绝".into(),
        Reason::NoPassword => "没有保存密码".into(),
        Reason::KeyUnreadable => "无法读取私钥文件".into(),
        Reason::KeyPassphrase => "私钥已加密，口令缺失或不正确".into(),
        Reason::NoAgent => "没有运行中的 SSH 代理，或代理中没有密钥".into(),
        Reason::NotLinux => "主机没有提供 POSIX shell 或 Linux 的 /proc".into(),
        Reason::TracingDenied => "读取事件跟踪需要管理员权限或 Performance Log Users 组".into(),
        Reason::NoFrames => "前台应用没有通过 Direct3D 呈现新画面".into(),
        Reason::NotificationsOff => "系统设置中已关闭本程序的通知".into(),
        Reason::Declined => "操作已取消".into(),
        Reason::HotkeyTaken => "快捷键已被其他程序占用".into(),
        Reason::ClipboardBusy => "剪贴板正被其他程序占用".into(),
        Reason::PathNotFound => "找不到文件或文件夹".into(),
    }
}

fn notice(notice: &Notice) -> String {
    match notice {
        Notice::Stopped => "监控服务已停止".into(),
        Notice::Busy => "操作繁忙，请稍后重试".into(),
        Notice::InvalidQuery => "历史查询范围无效".into(),
        Notice::InvalidSampling => "采样间隔必须为 0.5、1 或 2 秒".into(),
        Notice::InvalidHistory => "历史范围必须为 60、300 或 900 秒".into(),
        Notice::InvalidWindowSize => "窗口尺寸无效".into(),
        Notice::InvalidWindowPosition => "窗口位置无效".into(),
        Notice::InvalidValue => "部分配置无效，已恢复该项默认值".into(),
        Notice::InvalidType => "部分配置类型无效，已恢复该项默认值".into(),
        Notice::CorruptFile => {
            "配置文件损坏，已使用默认值；原文件保留，点击保存设置后才替换".into()
        }
        Notice::Unreadable(error) => format!("无法读取配置：{error}"),
        Notice::CorruptKept => "已保留损坏的配置，请在设置页面主动保存".into(),
        Notice::InvalidDirectory => "配置目录无效".into(),
        Notice::System(error) => error.clone(),
        Notice::Process(reason) => format!("进程操作失败：{}", self::reason(reason)),
        Notice::InvalidRemoteHost => "远程主机设置无效".into(),
        Notice::SecretUnsaved(reason) => format!("无法保存密码：{}", self::reason(reason)),
        Notice::AutostartUnchanged(reason) => {
            format!("无法更改开机自动启动：{}", self::reason(reason))
        }
        Notice::InvalidAlertRule => "告警规则无效".into(),
        Notice::NotificationFailed(reason) => {
            format!("无法推送通知：{}", self::reason(reason))
        }
        Notice::ElevationFailed(reason) => {
            format!("无法以管理员身份重新启动：{}", self::reason(reason))
        }
        Notice::InvalidHotkey => "快捷键无效".into(),
        Notice::InvalidQuickCommand => "快捷指令无效".into(),
        Notice::CommandFailed(reason) => format!("无法运行快捷指令：{}", self::reason(reason)),
        Notice::CommandStopFailed(reason) => {
            format!("无法终止快捷指令：{}", self::reason(reason))
        }
        Notice::InvalidRecordLimit => "保留数量不在可选范围内".into(),
        Notice::HotkeyUnavailable(shortcut, reason) => {
            let name = match shortcut {
                Shortcut::Screenshot => "截图",
                Shortcut::Note => "便签",
                Shortcut::Command(_) => "快捷指令",
            };
            format!("无法注册{name}快捷键：{}", self::reason(reason))
        }
        Notice::ScreenshotFailed(reason) => format!("截图失败：{}", self::reason(reason)),
        Notice::ScreenshotFile(error) => format!("截图文件操作失败：{error}"),
        Notice::ClipboardFailed(reason) => format!("无法使用剪贴板：{}", self::reason(reason)),
        Notice::OpenFailed(reason) => format!("无法打开：{}", self::reason(reason)),
        Notice::NoteFile(error) => format!("便签文件操作失败：{error}"),
    }
}

fn save_status(status: &SaveStatus) -> String {
    match status {
        SaveStatus::Loaded => "设置已加载".into(),
        SaveStatus::Recovered(warning) => notice(warning),
        SaveStatus::Saving => "设置已应用，正在保存…".into(),
        SaveStatus::Saved => "设置已保存".into(),
        SaveStatus::Unsaved(error) => format!("设置已应用，但未保存：{}", notice(error)),
    }
}
