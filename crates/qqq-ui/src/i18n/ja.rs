//! Japanese.

use super::{PageText, Strings};
use qqq_domain::{Reason, Shortcut};
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
    frames_page: PageText {
        nav: "フレーム",
        title: "フレームレートとフレームタイム",
        hint: "ゲーム中の前面アプリの滑らかさを確認",
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
    alerts_page: PageText {
        nav: "アラート",
        title: "監視アラート",
        hint: "指標がしきい値を超えたら Windows 通知でお知らせ",
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
    alerts_firing: |n| format!("{n} 件のアラートが発生中"),
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
    minutes_ago: |v| format!("{v} 分前"),
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
    foreground_app: "前面のアプリ",
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
    metric,

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
    autostart: "自動起動",
    autostart_option: "Windows へのサインイン時に QQQ Monitor を起動する",
    autostart_to_tray_option: "自動起動時はタスクトレイに格納する",
    frame_monitoring: "フレーム監視",
    frame_monitoring_option: "前面のアプリのフレームレートとフレームタイムを計測し、ミニバーにも FPS を表示する",
    apply_save: "適用して保存",
    runtime_status: "動作状況",
    batches: |received, dropped| format!("受信バッチ {received}  ·  破棄バッチ {dropped}"),
    history_usage: |used| format!("履歴データ {used} / 16 MiB"),
    collectors_ok: "すべてのデータソースが正常に動作しています",
    refresh_devices: "デバイスを再検出",
    about: "アプリ情報",
    about_intro: "看板娘「MINT ちゃん」が PC の様子を見守る、アニメ風ピクセルアートの Windows 向け多機能タスクマネージャーです。リアルタイムのパフォーマンス監視と履歴グラフ、プロセス管理、SSH による Linux ホストの監視、前面のアプリのフレームレート計測、監視アラートに加え、スクリーンショット、クリップボード履歴、付箋などのツールを備えています。",
    about_github: "GitHub ページ",

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

    fps: "フレームレート",
    frame_time: "フレームタイム",
    frame_time_max: "最長フレーム",
    fps_detail: |mean| format!("平均フレームタイム {mean}"),
    frame_time_detail: |longest| format!("最長 {longest}"),
    frames_target: "計測対象",
    frames_off: "フレーム監視はオフです。オンにすると、前面のアプリが 1 秒間に表示するフレーム数と隣り合うフレームの間隔をバックグラウンドで計測し、ミニバーにも FPS を表示します。",
    frames_denied: "システムのグラフィックス イベントを読み取るには管理者権限が必要です。管理者として再起動するか、現在のアカウントを Performance Log Users グループに追加して Windows にサインインし直してください。",
    frames_restart: "管理者として再起動",
    frames_waiting: "ゲームなどのアプリに切り替えると計測を始めます",
    fps_chart: "フレームレートの推移",
    frame_time_chart: "フレームタイムの推移",
    frames_note: "フレームレートは前面のアプリが 1 秒間に表示したフレーム数、フレームタイムは隣り合う表示の間隔です。Windows の DXGI と Direct3D 9 のイベント トレースから取得し、ゲームのプロセスには介入しません。このアプリに切り替えている間は直前のアプリを計測し続けます。DXGI を経由しない Vulkan や OpenGL のアプリはデータがない場合があります。",

    alert_add: "アラートを追加",
    alert_test: "テスト通知を送信",
    alert_test_body: "この通知が届いていれば、アラートは正常に通知されます。",
    alert_empty: "アラート ルールはまだありません。デバイスと指標を選んでしきい値を設定すると、値がしきい値を超えたときに Windows 通知が届きます。",
    alert_rules: "アラート ルール",
    alert_recent: "最近のアラート",
    alert_recent_empty: "今回の起動ではまだアラートは発生していません",
    alert_normal: "正常",
    alert_no_data: "データ待ち",
    alert_disabled: "無効",
    alert_pending: |left| format!("しきい値を超えています。{left}後に通知"),
    alert_firing: |held| format!("アラート発生中 · {held}継続"),
    alert_now: |value| format!("現在 {value}"),
    alert_above: "上回る",
    alert_below: "下回る",
    alert_condition: |above, threshold, hold| {
        let side = if above { "上回る" } else { "下回る" };
        match hold {
            Some(hold) => format!("{threshold} を{side} · {hold}継続"),
            None => format!("{threshold} を{side}"),
        }
    },
    alert_hold_none: "即時",
    alert_add_title: "アラート ルールの追加",
    alert_edit_title: "アラート ルールの編集",
    alert_device: "デバイス",
    alert_metric: "指標",
    alert_when: "条件",
    alert_threshold: "しきい値",
    alert_hold: "継続時間",
    alert_enabled: "このルールを有効にする",
    alert_pick_device: "デバイスを選択",
    alert_device_missing: "未接続",
    alert_missing: "デバイスと指標を選んでください",
    alert_bad_threshold: "しきい値には数値を入力してください",
    alert_save: "保存",
    alert_edit: "編集",
    alert_remove: "削除",
    alert_title: |device, metric, above| {
        format!(
            "{device} の{metric}が{}",
            if above {
                "高くなっています"
            } else {
                "低くなっています"
            }
        )
    },
    alert_body: |value, threshold, above, hold| {
        let side = if above {
            "上回って"
        } else {
            "下回って"
        };
        match hold {
            Some(hold) => {
                format!("現在 {value}。しきい値 {threshold} を {hold}継続して{side}います")
            }
            None => format!("現在 {value}。しきい値 {threshold} を{side}います"),
        }
    },
    alert_note: "ルールは各データソースの最新の有効な値を読み取ります。値がしきい値を超えた状態が継続時間だけ続くと Windows 通知を送り、反対側に戻った状態が同じ時間続くと正常に戻ります。同じルールの通知は 1 分に 1 回までです。ウィンドウをトレイに格納したりミニバーにドッキングしたりしていても通知し、通知をクリックするとメイン ウィンドウが開きます。",

    tools_page: PageText {
        nav: "ツール",
        title: "ツール",
        hint: "スクリーンショット、クリップボード履歴、付箋、クイックコマンドなどの便利な機能",
    },
    tool_screenshots: "キャプチャ",
    tool_clipboard: "クリップボード",
    tool_notes: "付箋",
    tool_commands: "クイックコマンド",
    tray_screenshot: "スクリーンショット",
    hours_ago: |v| format!("{v} 時間前"),
    shot_take: "キャプチャ開始",
    shot_hint: "ドラッグで範囲を選択 · クリックで画面全体 · 右クリックか Esc でキャンセル",
    shot_folder: "フォルダーを開く",
    shot_hotkey: "ショートカット",
    shot_hotkey_option: |key| format!("{key} でキャプチャ"),
    shot_hotkey_change: "変更",
    shot_hotkey_press: "新しいショートカットを押してください（Esc でキャンセル）",
    shot_hotkey_invalid: "Ctrl か Alt と英字・数字の組み合わせ、または F1〜F24 を使えます",
    keep: "保存数",
    shot_count: |n| format!("{n} 枚"),
    shot_recent: "最近のスクリーンショット",
    shot_empty: "スクリーンショットはまだありません。ショートカットを押すか「キャプチャ開始」をクリックしてください。",
    shot_loading: "スクリーンショットを読み込み中…",
    shot_busy: "スクリーンショットの保存が混み合っています。保存待ちの書き込み後に再試行してください",
    shot_memory_full: "画像用メモリが上限に達しました。処理中のスクリーンショットの完了後に再試行してください",
    shot_too_large: "スクリーンショットの画像が大きすぎます",
    shot_storage_unavailable: "スクリーンショットの保存機能を利用できません",
    shot_note: |folder| {
        format!(
            "マウスを離すと、選択した範囲がクリップボードにコピーされ、PNG として {folder} に保存されます。保存数を超えると古いものから削除します。「キャプチャ開始」はメイン ウィンドウを隠してからキャプチャします。"
        )
    },
    copy: "コピー",
    open: "開く",
    delete: "削除",

    clip_option: "クリップボードにコピーしたテキスト・画像・ファイルを記録",
    clip_off: "クリップボード履歴はオフです。オンにすると、それ以降にコピーした内容を記録します。",
    clip_clear: "履歴を消去",
    clip_count: |n| format!("{n} 件"),
    clip_recent: "クリップボード履歴",
    clip_empty: "まだ記録はありません。テキスト・画像・ファイルをコピーするとここに表示されます。",
    clip_text: |n| format!("テキスト · {n} 文字"),
    clip_picture: |w, h| format!("画像 · {w} × {h}"),
    clip_files: |n| format!("{n} 個のファイル"),
    clip_more: |n| format!("ほか {n} 件"),
    clip_note: "項目をクリックするとクリップボードにコピーし直します。新しい項目ほど上に並びます。履歴はメモリにだけ保持し、プログラムの終了時に消去します。パスワード マネージャーが記録対象外とした内容はスキップし、画像は合計 256 MiB までメモリを使います。",

    tray_note: "付箋",
    note_title: "QQQ 付箋",
    note_hotkey_option: |key| format!("{key} で付箋を表示・非表示"),
    hotkey_taken: "このキーの組み合わせはほかのショートカットで使用中です",
    hotkey_none: "ショートカット未設定",
    hotkey_set: "設定",
    note_open: "付箋を開く",
    note_new: "新しい付箋",
    note_list: "すべての付箋",
    note_write: "編集",
    note_read: "プレビュー",
    note_delete: "付箋を削除",
    note_hide: "付箋を隠す",
    note_untitled: "無題の付箋",
    note_hint: "ここに書き込めます（Markdown 対応）",
    note_read_empty: "この付箋は空です。ここをダブルクリックするか Ctrl+E を押して書き始めましょう。",
    note_delete_confirm: "この付箋を削除しますか？",
    note_delete_confirm_short: "削除する",
    note_chars: |n| format!("{n} 文字"),
    note_saving: "保存中…",
    note_saved: "保存済み",
    note_save_failed: |error| format!("保存できません：{error}"),
    note_storage_unavailable: "付箋の保存機能を利用できません",
    note_count: |n| format!("{n} 件"),
    note_empty: "付箋はまだありません。ショートカットを押すか「付箋を開く」をクリックして書き始めましょう。",
    note_loading: "付箋を読み込み中…",
    note_about: |folder| {
        format!(
            "付箋ウィンドウはほかのウィンドウより常に手前に表示されます。ショートカットで表示し、付箋の中でもう一度押すと隠れます。タイトル バーをドラッグして移動し、右下の角をドラッグしてサイズを変更します。内容は Markdown で、Ctrl+E で編集とプレビューを切り替え、Ctrl+N で新しい付箋を作ります。プレビューではタスクにチェックを付けたり Web リンクを開いたりできます。付箋は 1 件ずつ .md ファイルとして {folder} に保存され、入力が止まると自動で保存されます。"
        )
    },

    command_hotkey_option: |key| format!("{key} で実行"),
    command_new: "新しいコマンド",
    command_list: "すべてのクイックコマンド",
    command_count: |n| format!("{n} 件"),
    command_empty: "クイックコマンドはまだありません。「新しいコマンド」をクリックして、実行する PowerShell コマンドを入力してください。",
    command_run: "実行",
    command_stop: "終了",
    command_running: |n| format!("実行中 · セッション {n} 個"),
    command_delete_ends: |n| {
        format!("削除すると、バックグラウンドで実行中のセッション {n} 個も終了します")
    },
    command_sound_option: "クイックコマンドの実行時に効果音を鳴らす",
    command_balloon: |n| format!("{n} 個のセッションで実行を開始しました"),
    command_edit: "編集",
    command_add_title: "新しいクイックコマンド",
    command_edit_title: "クイックコマンドの編集",
    command_name: "名前",
    command_lines: "コマンド",
    command_lines_hint: "1 行に 1 つの PowerShell コマンド",
    command_sessions: "セッション",
    command_shared: "1 つのセッションで順に実行",
    command_separate: "コマンドごとに別のセッション",
    command_window: "ウィンドウ",
    command_window_option: "PowerShell ウィンドウを表示し、終了後も開いたままにする",
    command_directory: "作業フォルダー",
    command_directory_hint: "空欄ならユーザー フォルダーで開始",
    command_save: "保存",
    command_missing_name: "名前を入力してください",
    command_missing_lines: "コマンドを 1 つ以上入力してください",
    command_too_long: |lines, bytes| {
        format!("コマンドが多すぎるか長すぎます（最大 {lines} 行、半角で合計約 {bytes} 文字）")
    },
    command_bad_directory: "このフォルダーが見つかりません。完全なパスを入力してください",
    command_summary: |shared, lines, window| {
        format!(
            "{} · コマンド {lines} 個 · {}",
            if shared {
                "1 つのセッション"
            } else {
                "コマンドごとのセッション"
            },
            if window {
                "ウィンドウ表示"
            } else {
                "バックグラウンド"
            }
        )
    },
    command_about: "クイックコマンドは Windows PowerShell で実行します。「1 つのセッションで順に実行」では、すべてのコマンドを 1 つのセッションで順番に実行するので、前のコマンドで設定した変数や現在の場所を後のコマンドでもそのまま使えます。「コマンドごとに別のセッション」では、各コマンドをそれぞれのセッションで同時に開始します。ウィンドウを表示すると、コマンドの終了後もウィンドウが開いたままになり、出力を確認したり続けて入力したりできます。表示しない場合はバックグラウンドで実行し、コマンドが終わるとセッションも終了します。セッションの実行中は、実行中のセッション数と「終了」ボタンが表示され、クリックするとすべてのセッションと、そこから起動したプログラムを終了します。実行時には効果音が鳴ります。ショートカットはほかのアプリからも使え、ウィンドウをトレイに格納したりミニバーにドッキングしたりしていても有効で、そのときはトレイのバルーンで実行結果も知らせます。コマンドの「設定」をクリックしてキーを押すと割り当てられます。",
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
        "process_id" => "プロセス ID",
        "present_api" => "表示 API",
        _ => return None,
    })
}

fn metric(name: &str) -> Option<&'static str> {
    Some(match name {
        "usage" => "使用率",
        "iowait" => "I/O 待ち",
        "used" => "使用量",
        "available" => "空き容量",
        "swap_usage" => "スワップ使用率",
        "memory_usage" => "GPU メモリ使用率",
        "memory_used" => "GPU メモリ使用量",
        "read" => "読み取り速度",
        "write" => "書き込み速度",
        "receive" => "受信速度",
        "send" => "送信速度",
        "load1" => "1 分間の平均負荷",
        "load5" => "5 分間の平均負荷",
        "load15" => "15 分間の平均負荷",
        "processes" => "プロセス数",
        "fps" => "フレームレート",
        "frame_time" => "平均フレームタイム",
        "frame_time_max" => "最長フレーム",
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
        Reason::TracingDenied => {
            "イベント トレースの読み取りには管理者権限か Performance Log Users グループが必要です"
                .into()
        }
        Reason::NoFrames => "前面のアプリが Direct3D で新しいフレームを表示していません".into(),
        Reason::NotificationsOff => "システムの設定でこのアプリの通知がオフになっています".into(),
        Reason::Declined => "操作は取り消されました".into(),
        Reason::HotkeyTaken => "ショートカットはほかのアプリが使用しています".into(),
        Reason::ClipboardBusy => "クリップボードはほかのアプリが使用中です".into(),
        Reason::PathNotFound => "ファイルまたはフォルダーが見つかりません".into(),
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
        Notice::AutostartUnchanged(reason) => {
            format!("自動起動を変更できません：{}", self::reason(reason))
        }
        Notice::InvalidAlertRule => "アラート ルールが無効です".into(),
        Notice::NotificationFailed(reason) => {
            format!("通知を表示できません：{}", self::reason(reason))
        }
        Notice::ElevationFailed(reason) => {
            format!("管理者として再起動できません：{}", self::reason(reason))
        }
        Notice::InvalidHotkey => "ショートカットが無効です".into(),
        Notice::InvalidQuickCommand => "クイックコマンドが無効です".into(),
        Notice::CommandFailed(reason) => {
            format!(
                "クイックコマンドを実行できません：{}",
                self::reason(reason)
            )
        }
        Notice::CommandStopFailed(reason) => {
            format!(
                "クイックコマンドを終了できません：{}",
                self::reason(reason)
            )
        }
        Notice::InvalidRecordLimit => "保存数が選択肢にありません".into(),
        Notice::HotkeyUnavailable(shortcut, reason) => {
            let name = match shortcut {
                Shortcut::Screenshot => "スクリーンショット",
                Shortcut::Note => "付箋",
                Shortcut::Command(_) => "クイックコマンド",
            };
            format!(
                "{name}のショートカットを登録できません：{}",
                self::reason(reason)
            )
        }
        Notice::ScreenshotFailed(reason) => {
            format!("キャプチャできません：{}", self::reason(reason))
        }
        Notice::ScreenshotFile(error) => format!("スクリーンショットのファイル操作に失敗：{error}"),
        Notice::ClipboardFailed(reason) => {
            format!("クリップボードを使用できません：{}", self::reason(reason))
        }
        Notice::OpenFailed(reason) => format!("開けません：{}", self::reason(reason)),
        Notice::NoteFile(error) => format!("付箋ファイルの操作に失敗：{error}"),
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
