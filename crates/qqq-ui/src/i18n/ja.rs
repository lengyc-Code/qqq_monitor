//! Japanese.

use super::{PageText, Strings};
use qqq_domain::Reason;
use qqq_engine::{Notice, SaveStatus};

pub const STRINGS: Strings = Strings {
    window_title: "QQQ Monitor · パフォーマンス監視",
    tagline: "パフォーマンス監視",
    startup_failed: "QQQ Monitor を起動できませんでした",
    overview_page: PageText {
        nav: "概要",
        title: "システム概要",
        hint: "PC の動作状況をリアルタイムで確認",
    },
    cpu_page: PageText {
        nav: "CPU",
        title: "プロセッサ",
        hint: "全体の負荷と各論理コアの状況",
    },
    memory_page: PageText {
        nav: "メモリ",
        title: "物理メモリ",
        hint: "利用可能な物理メモリと使用量の推移",
    },
    gpu_page: PageText {
        nav: "GPU",
        title: "グラフィックス",
        hint: "GPU 使用率、各エンジンの負荷と GPU メモリ",
    },
    disk_page: PageText {
        nav: "ディスク",
        title: "ディスクとストレージ",
        hint: "物理ディスクの読み書きとローカルボリュームの容量",
    },
    network_page: PageText {
        nav: "ネットワーク",
        title: "ネットワーク通信",
        hint: "選択したアダプターのリアルタイム送受信",
    },
    processes_page: PageText {
        nav: "プロセス",
        title: "プロセス",
        hint: "Windows と実行中の WSL ディストリビューションのプロセスを管理",
    },
    remote_page: PageText {
        nav: "リモート",
        title: "リモートホスト",
        hint: "SSH で Linux ホストのシステム情報と主要な指標を監視",
    },
    settings_page: PageText {
        nav: "設定",
        title: "設定",
        hint: "使い方に合わせてモニターを調整",
    },

    history_range: "履歴の範囲",
    seconds: |v| format!("{v} 秒"),
    minutes: |v| format!("{v} 分"),
    running: "監視中",
    degraded: "一部のデータを取得できません",
    timing: |sampling, session| format!("サンプリング {sampling:.1}s  ·  セッション {session:.0}s"),
    minibar: "ミニバー",
    dock_top: "画面の上端にドッキング",
    dock_left: "画面の左端にドッキング",
    dock_right: "画面の右端にドッキング",
    restore_window: "ウィンドウを元に戻す",

    tray_open: "メインウィンドウを表示",
    tray_exit: "終了",

    days: |v| format!("{v} 日"),
    state_ready: "リアルタイム",
    state_stale: "データが古い",
    state_failed: "取得失敗",
    state_unsupported: "非対応",
    state_offline: "切断",
    state_waiting: "取得中",
    seconds_ago: |v| format!("{v:.0} 秒前"),
    last_valid: "最後の有効データ",
    now: "現在",
    chart_waiting: "有効なサンプルを待っています。データがない区間はグラフが途切れます",
    device_offline: "デバイスは切断されています",
    offline_suffix: "（切断）",
    finding_devices: "デバイスを検出中",
    waiting_samples: "サンプル待ち",
    click_for_details: "クリックで詳細を表示",
    list_separator: "、",
    labeled: |label, text| format!("{label}：{text}"),
    used_of: |used, total| format!("使用中 {used} / {total}"),

    processor: "プロセッサ",
    physical_memory: "物理メモリ",
    this_pc: "この PC",
    physical_disk: "物理ディスク",
    graphics_card: "グラフィックスカード",
    adapter: "アダプター",
    logical_core: |n| format!("論理コア {n}"),
    detail,
    physical_adapter: "物理アダプター",
    virtual_adapter: "仮想またはソフトウェア インターフェイス",
    dedicated_memory: "専用 GPU メモリ",
    shared_memory: "共有 GPU メモリ",
    reason,
    notice,
    save_status,

    memory: "メモリ",
    gpu_memory: "GPU メモリ",
    memory_short: "メモリ",
    gpu_memory_short: "VRAM",
    read: "読み取り",
    write: "書き込み",
    receive: "受信",
    send: "送信",
    logical_cores: "論理コア",

    mint_name: "看板娘 · MINT ちゃん",
    mint_hover: "看板娘の MINT ちゃんは CPU とメモリの負荷で表情が変わります",
    mint_waiting: "センサー起動中…\nちょっと待っててね～",
    mint_trouble: "取れないデータが…\n「設定」を見てみてね",
    mint_cpu_hot: |cpu| format!("CPU {cpu:.0}%！\nもう煙が出そう >_<"),
    mint_memory_full: |memory| format!("メモリ {memory:.0}% 使用中\nもう入りきらないよ！"),
    mint_busy: |cpu, memory| format!("ちょっと忙しいかも～\nCPU {cpu:.0}%  メモリ {memory:.0}%"),
    mint_relaxed: |cpu| format!("のんびり～CPU {cpu:.0}%\nお茶にしよっか"),
    mint_calm: |cpu, memory| format!("すべて順調！\nCPU {cpu:.0}%  メモリ {memory:.0}%"),

    cores_detail: |cores| format!("論理コア {cores}"),
    disk_read: "ディスク読み取り",
    network_receive: "ネットワーク受信",
    cpu_memory_chart: "CPU とメモリの使用率の推移",
    disk_io: "ディスクの読み書き",
    network_io: "ネットワークの送受信",
    device_profile: "デバイス情報",
    total_memory: "メモリ総量",
    current_disk: "ディスク",
    current_adapter: "アダプター",
    uptime: "稼働時間",

    cpu_total: "全体の使用率",
    cpu_total_detail: "全論理コアの平均負荷",
    cpu_busiest: "最も忙しいコア",
    cpu_overall: "CPU 全体",
    cpu_selected_core: "選択中のコア",
    cpu_chart: "CPU 使用率",
    cpu_cores_hint: "コアをクリックすると履歴を表示",

    memory_in_use: "使用中",
    share_of_total: |share| format!("総量の {share}"),
    available: "利用可能",
    allocatable: |share| format!("あと {share} を割り当て可能"),
    memory_total: "物理メモリの総量",
    memory_total_detail: "システムで使用できる物理メモリ",
    memory_chart: "物理メモリ使用率の推移",
    memory_usage: "メモリ使用率",
    memory_note: "システムで使用できる物理メモリを集計します。使用中のメモリとコミット済みメモリは別の指標です。",

    gpu_usage: "GPU 使用率",
    engines_waiting: "エンジンのデータを待っています",
    busiest_engine: |engine, load| format!("最も忙しいエンジン {engine} · {load}"),
    gpu_memory_used: |kind, used, total| format!("{kind} 使用中 {used} / {total}"),
    gpu_chart: "GPU と GPU メモリの使用率",
    engines_title: "エンジンの負荷",
    engines_hint: "種類ごとに最も忙しいエンジンを表示。クリックすると履歴を表示",
    engines_missing: "エンジンのデータを待っています。エンジンカウンターのないカードはメモリのみ表示します",
    engine: |name| format!("{name} エンジン"),
    engine_kind,
    gpu_memory_title: "GPU メモリの内訳",
    gpu_memory_hint: "専用 GPU メモリはカード上に、共有 GPU メモリはシステムメモリにあります",
    total_share: |total, share| format!("合計 {total}  ·  使用率 {share}"),
    gpu_note: "GPU 使用率はタスク マネージャーと同じ方法で求めます。同じエンジンの負荷をプロセス間で合計し、最も忙しいエンジンの値を採用します。種類の異なるエンジンは合算しません。",

    disk_read_detail: "選択した物理ディスクの読み取り速度",
    disk_write_detail: "選択した物理ディスクの書き込み速度",
    disk_chart: "物理ディスクの読み書き速度",
    volumes_title: "ローカルボリュームの容量",
    volumes_hint: "ボリューム容量と物理ディスクの I/O は別々に集計します",
    volumes_waiting: "ローカルボリュームを検出中",
    volume_space: |free, total| format!("空き {free}  ·  合計 {total}"),
    no_label: "ラベルなし",
    unknown_file_system: "不明なファイルシステム",

    session_received: |bytes| format!("このセッションの受信 {bytes}"),
    session_sent: |bytes| format!("このセッションの送信 {bytes}"),
    network_chart: "ネットワーク送受信の推移",
    network_note: "選択した 1 つのインターフェイスのみを集計するため、物理アダプターと VPN の通信を二重に数えません。セッションの通信量は、このアプリが観測したカウンターの有効な増加分です。",

    preferences: "監視の設定",
    language: "表示言語",
    follow_system: "システムに従う",
    sampling_interval: "サンプリング間隔",
    default_range: "既定の履歴範囲",
    animations: "アニメーション",
    animations_option: "軽いアニメーションと看板娘の揺れを有効にする",
    edge_docking: "端へのドッキング",
    edge_docking_option: "上端か左右の端へドラッグするとミニバーになる",
    minimize_to_tray: "トレイに最小化",
    minimize_to_tray_option: "最小化するとタスクトレイにアイコンだけを残す",
    apply_save: "適用して保存",
    runtime_status: "動作状況",
    batches: |received, dropped| format!("受信バッチ {received}  ·  破棄バッチ {dropped}"),
    history_usage: |used| format!("履歴データ {used} / 16 MiB"),
    collectors_ok: "すべてのデータソースが正常に動作しています",
    refresh_devices: "デバイスを再検出",
    about_mint: "看板娘の MINT ちゃんは CPU とメモリの負荷に合わせて、のんびり・おだやか・いそがしい・オーバーロードの表情に変わります。",
    about_animations: "アニメーションをオフにしても表情は負荷に合わせて変わりますが、揺れなくなり、そのための再描画も行いません。",

    process_name: "名前",
    process_user: "ユーザー",
    process_memory: "メモリ",
    process_threads: "スレッド",
    process_priority: "優先度",
    process_command: "コマンドライン",
    process_search: "名前、PID、ユーザー、コマンドラインで検索",
    process_count: |shown, total| {
        if shown == total {
            format!("{total} 個のプロセス")
        } else {
            format!("{total} 個中 {shown} 個のプロセス")
        }
    },
    processes_loading: "プロセスを読み込んでいます…",
    processes_none: "一致するプロセスはありません",
    processes_failed: |reason| format!("プロセスを読み込めません：{reason}"),
    wsl_hint: "実行中の WSL ディストリビューションがここに表示されます",
    end_process: "プロセスを終了",
    kill_process: "強制終了",
    end_confirm: |name, pid| format!("「{name}」（PID {pid}）を終了しますか？"),
    end_warning: "プロセスの保存されていないデータは失われます。",
    kill_warning: "強制終了するとプロセスはすぐに停止し、保存されていないデータは失われます。",
    cancel: "キャンセル",
    priorities: ["低", "通常以下", "通常", "通常以上", "高", "リアルタイム"],

    remote_add: "ホストを追加",
    remote_empty: "リモートホストはまだありません。SSH でログインできる Linux ホストを追加すると、システム情報、CPU、メモリ、ディスク、ネットワーク、ファイルシステムをここで確認できます。",
    remote_connecting: "接続中…",
    remote_connected: "接続済み",
    remote_failed: |reason| format!("接続できません：{reason}"),
    remote_retrying: "まもなく再試行します",
    remote_key_unknown: |fingerprint| {
        format!(
            "このホストへの初めての接続です。ホスト鍵のフィンガープリントをサーバー管理者に確認し、一致した場合のみ信頼してください：\n{fingerprint}"
        )
    },
    remote_key_changed: |fingerprint| {
        format!(
            "ホスト鍵が以前に信頼した鍵と異なります。サーバーが再インストールされたか、なりすましの可能性があります。確認できた場合のみ新しい鍵を信頼してください：\n{fingerprint}"
        )
    },
    remote_trust: "信頼して接続",
    remote_trust_new: "新しい鍵を信頼",
    remote_edit: "編集",
    remote_reconnect: "再接続",
    remote_remove: "削除",
    remote_remove_confirm: |name| format!("リモートホスト「{name}」を削除しますか？"),
    remote_remove_warning: "資格情報マネージャーに保存したパスワードまたはパスフレーズと、収集したデータも削除されます。",
    remote_auth_password: "パスワード",
    remote_auth_key: "秘密鍵ファイル",
    remote_auth_agent: "SSH エージェント",
    remote_add_title: "リモートホストを追加",
    remote_edit_title: "リモートホストを編集",
    remote_import: "SSH 設定から読み込む",
    remote_name: "名前",
    remote_name_hint: "省略可。空欄ならアドレスを表示",
    remote_address: "アドレス",
    remote_address_hint: "ホスト名または IP アドレス",
    remote_port: "ポート",
    remote_user: "ユーザー名",
    remote_auth: "ログイン方法",
    remote_password: "パスワード",
    remote_password_keep: "空欄なら保存済みのパスワードを使用",
    remote_key_file: "秘密鍵ファイル",
    remote_key_hint: "例：~/.ssh/id_ed25519",
    remote_passphrase: "パスフレーズ",
    remote_passphrase_hint: "暗号化されていない鍵では空欄",
    remote_passphrase_keep: "空欄なら保存済みのパスフレーズを使用",
    remote_agent_hint: "Windows の OpenSSH Authentication Agent サービスまたは Pageant に読み込まれた鍵を使います。",
    remote_secret_note: "パスワードとパスフレーズは Windows 資格情報マネージャーに保存され、設定ファイルには書き込まれません。",
    remote_save: "保存して接続",
    remote_missing: |field| format!("{field}を入力してください"),
    remote_bad_port: "ポートは 1〜65535 の数字で指定してください",
    remote_hostname: "ホスト名",
    remote_load: "ロードアベレージ",
    remote_cpu_detail: |iowait, processes| format!("I/O 待ち {iowait} · プロセス {processes}"),
    remote_swap: "スワップ",
    remote_no_swap: "なし",
    remote_volumes_title: "ファイルシステム",
    remote_volumes_hint: "ローカルファイルシステムの容量。使用率は df と同じ基準",
    remote_cores_hint: "各論理コアの現在の負荷",
    yes: "はい",
};

fn detail(key: &str) -> Option<&'static str> {
    Some(match key {
        "logical_cores" => "論理コア",
        "os" => "OS",
        "description" => "説明",
        "interface_guid" => "インターフェイス GUID",
        "interface_type" => "インターフェイスの種類",
        "link_speed" => "リンク速度",
        "vendor" => "ベンダー",
        "memory_type" => "メモリの種類",
        "adapter_luid" => "アダプター LUID",
        "counter_instance" => "カウンター インスタンス",
        "volume_label" => "ボリューム ラベル",
        "file_system" => "ファイルシステム",
        "volume_guid" => "ボリューム GUID",
        "kernel" => "カーネル",
        "architecture" => "アーキテクチャ",
        "default_route" => "デフォルトルート",
        "device" => "デバイス",
        _ => return None,
    })
}

fn engine_kind(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "copy" => "コピー",
        "videodecode" => "ビデオデコード",
        "videoencode" => "ビデオエンコード",
        "videoprocessing" => "ビデオ処理",
        "compute" => "コンピュート",
        "graphics" => "グラフィックス",
        "legacyoverlay" => "レガシーオーバーレイ",
        "overlay" => "オーバーレイ",
        "security" => "セキュリティ",
        "ofa" => "オプティカルフロー",
        _ => return None,
    })
}

fn reason(reason: &Reason) -> String {
    match reason {
        Reason::NoGpuCounters => {
            "このグラフィックスカードのパフォーマンス カウンターは提供されていません".into()
        }
        Reason::NoGpuMemoryCounters => "GPU メモリのカウンターを利用できません".into(),
        Reason::VolumeUnavailable => "ボリューム容量を一時的に取得できません".into(),
        Reason::NoMemoryTotal => "物理メモリの総量を取得できません".into(),
        Reason::OutOfRange => "取得値が有効範囲外です".into(),
        Reason::CounterUnit => "カウンターの単位が無効です".into(),
        Reason::Disconnected => "デバイスは切断されています".into(),
        Reason::Panicked => "取得スレッドで異常が発生したため、再作成しています".into(),
        Reason::KeyConflict => "メトリック キーが他のコレクターと競合しています".into(),
        Reason::InstancesChanging => {
            "カウンター インスタンスの変化が激しく、一時的に読み取れません".into()
        }
        Reason::CallFailed { api, code } => {
            format!("{api} が失敗しました（エラーコード 0x{code:08X}）")
        }
        Reason::BadData { api } => format!("{api} が無効なデータを返しました"),
        Reason::ProcessGone => "プロセスは終了しています".into(),
        Reason::AccessDenied => {
            "アクセスが拒否されました。管理者として実行してからやり直してください".into()
        }
        Reason::HostStopped => "WSL ディストリビューションは停止しています".into(),
        Reason::AddressNotFound => "ホスト名を解決できません".into(),
        Reason::ConnectionRefused => {
            "接続が拒否されました。アドレス、ポート、SSH サービスを確認してください".into()
        }
        Reason::TimedOut => "接続がタイムアウトしました".into(),
        Reason::ConnectionLost => "接続が切れました".into(),
        Reason::Handshake => {
            "SSH ハンドシェイクに失敗しました。双方が対応するアルゴリズムがありません".into()
        }
        Reason::HostKeyUnknown { .. } => "ホスト鍵がまだ信頼されていません".into(),
        Reason::HostKeyChanged { .. } => "ホスト鍵が信頼済みの鍵と異なります".into(),
        Reason::AuthFailed => "ユーザー名、パスワード、または鍵が拒否されました".into(),
        Reason::NoPassword => "パスワードが保存されていません".into(),
        Reason::KeyUnreadable => "秘密鍵ファイルを読み込めません".into(),
        Reason::KeyPassphrase => {
            "秘密鍵が暗号化されており、パスフレーズがないか正しくありません".into()
        }
        Reason::NoAgent => "SSH エージェントが動作していないか、鍵がありません".into(),
        Reason::NotLinux => "ホストに POSIX シェルまたは Linux の /proc がありません".into(),
    }
}

fn notice(notice: &Notice) -> String {
    match notice {
        Notice::Stopped => "監視サービスは停止しています".into(),
        Notice::Busy => "処理が混み合っています。しばらくしてから再試行してください".into(),
        Notice::InvalidQuery => "履歴の照会範囲が無効です".into(),
        Notice::InvalidSampling => "サンプリング間隔は 0.5、1、2 秒のいずれかです".into(),
        Notice::InvalidHistory => "履歴の範囲は 60、300、900 秒のいずれかです".into(),
        Notice::InvalidWindowSize => "ウィンドウのサイズが無効です".into(),
        Notice::InvalidWindowPosition => "ウィンドウの位置が無効です".into(),
        Notice::InvalidValue => "一部の設定が無効だったため、既定値に戻しました".into(),
        Notice::InvalidType => "一部の設定の型が無効だったため、既定値に戻しました".into(),
        Notice::CorruptFile => {
            "設定ファイルが破損しているため既定値を使用しています。元のファイルは設定を保存するまで残ります".into()
        }
        Notice::Unreadable(error) => format!("設定を読み込めません：{error}"),
        Notice::CorruptKept => {
            "破損した設定ファイルを残しています。設定ページから保存すると置き換えます".into()
        }
        Notice::InvalidDirectory => "設定フォルダーが無効です".into(),
        Notice::System(error) => error.clone(),
        Notice::Process(reason) => format!("プロセスの操作に失敗しました：{}", self::reason(reason)),
        Notice::InvalidRemoteHost => "リモートホストの設定が無効です".into(),
        Notice::SecretUnsaved(reason) => {
            format!("パスワードを保存できません：{}", self::reason(reason))
        }
    }
}

fn save_status(status: &SaveStatus) -> String {
    match status {
        SaveStatus::Loaded => "設定を読み込みました".into(),
        SaveStatus::Recovered(warning) => notice(warning),
        SaveStatus::Saving => "設定を適用しました。保存中…".into(),
        SaveStatus::Saved => "設定を保存しました".into(),
        SaveStatus::Unsaved(error) => {
            format!(
                "設定を適用しましたが、保存できませんでした：{}",
                notice(error)
            )
        }
    }
}
