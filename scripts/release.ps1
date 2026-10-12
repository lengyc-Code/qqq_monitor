<#
.SYNOPSIS
发布 Cargo.toml 中的当前版本：检查、构建、打包，然后推送并创建 GitHub Release。

.DESCRIPTION
版本号取自根包 qqq_monitor，标签为 v<版本>。脚本依次执行：

1. 确认工作区干净、位于 main 且与 origin/main 同步，标签与 Release 均不存在，gh 已登录。
2. 运行 fmt、Clippy 与 Release 测试。
3. 以静态链接的 C 运行库构建 x86_64-pc-windows-msvc Release 版，确认程序不依赖 VC++
   运行库，并运行冒烟测试。
4. 把程序、LICENSE、字体许可与三份 README 打包为
   target\dist\v<版本>\qqq_monitor-v<版本>-x86_64-pc-windows-msvc.zip，并生成 SHA256SUMS.txt。
5. 推送 main 与附注标签 v<版本>，创建 Release 并上传 ZIP 与 SHA256SUMS.txt。

.PARAMETER NotesFile
Release 说明的 Markdown 文件，发布时必填。

.PARAMETER PackageOnly
只检查、构建和打包，保留 Git 与 GitHub 原样；工作区可以有未提交的改动。

.PARAMETER Draft
以草稿创建 Release。

.PARAMETER SkipChecks
跳过 fmt、Clippy、测试与冒烟测试。

.EXAMPLE
.\scripts\release.ps1 -PackageOnly

.EXAMPLE
.\scripts\release.ps1 -NotesFile ..\notes-v0.3.0.md
#>
[CmdletBinding()]
param(
    [string]$NotesFile,
    [switch]$PackageOnly,
    [switch]$Draft,
    [switch]$SkipChecks
)

Set-StrictMode -Version 3
$ErrorActionPreference = 'Stop'

$Branch = 'main'
$Target = 'x86_64-pc-windows-msvc'
$Root = Split-Path -Parent $PSScriptRoot

function Write-Step([string]$Text) {
    Write-Host "==> $Text" -ForegroundColor Cyan
}

# Runs a command-line tool and fails on a non-zero exit code. Tools report progress on
# standard error, so only the exit code decides success.
function Invoke-Tool([string]$File, [string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$File $($Arguments -join ' ') 失败，退出码 $LASTEXITCODE"
    }
}

# Standard output of a command-line tool, with its exit code in $LASTEXITCODE.
function Get-ToolOutput([string]$File, [string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    & $File @Arguments 2> $null
}

# Relative to the caller's directory, before the script moves to the repository root.
if ($NotesFile) { $NotesFile = (Resolve-Path $NotesFile).Path }
Push-Location $Root
$savedFlags = $env:RUSTFLAGS
try {
    $metadata = Get-ToolOutput cargo @('metadata', '--no-deps', '--format-version', '1', '--locked')
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata 失败，Cargo.lock 可能需要更新' }
    $version = (($metadata | Out-String | ConvertFrom-Json).packages |
        Where-Object { $_.name -eq 'qqq_monitor' }).version
    $tag = "v$version"
    $name = "qqq_monitor-$tag-$Target"
    $output = Join-Path $Root "target\dist\$tag"
    Write-Step "发布 QQQ Monitor $tag"

    if (-not $PackageOnly) {
        Write-Step '检查发布条件'
        if (-not $NotesFile) { throw '发布需要 -NotesFile 指定 Release 说明' }
        if (Get-ToolOutput git @('status', '--porcelain')) {
            throw '工作区有未提交的改动，请先提交'
        }
        $current = Get-ToolOutput git @('rev-parse', '--abbrev-ref', 'HEAD')
        if ($current -ne $Branch) { throw "当前分支是 $current，发布需要在 $Branch 上进行" }
        Invoke-Tool git @('fetch', '--quiet', 'origin', $Branch)
        $behind = Get-ToolOutput git @('rev-list', '--count', "HEAD..origin/$Branch")
        if ([int]$behind -gt 0) { throw "$Branch 落后 origin/$Branch $behind 个提交，请先合并" }
        Get-ToolOutput git @('rev-parse', '--quiet', '--verify', "refs/tags/$tag") | Out-Null
        if ($LASTEXITCODE -eq 0) { throw "本地已有标签 $tag" }
        if (Get-ToolOutput git @('ls-remote', '--tags', 'origin', "refs/tags/$tag")) {
            throw "GitHub 上已有标签 $tag"
        }
        Invoke-Tool gh @('auth', 'status')
        Get-ToolOutput gh @('release', 'view', $tag) | Out-Null
        if ($LASTEXITCODE -eq 0) { throw "GitHub 上已有 Release $tag" }
    }

    if (-not $SkipChecks) {
        Write-Step 'fmt、Clippy 与测试'
        Invoke-Tool cargo @('fmt', '--all', '--', '--check')
        Invoke-Tool cargo @('clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings')
        Invoke-Tool cargo @('test', '--workspace', '--release', '--locked')
    }

    Write-Step "构建 $Target Release 版（静态链接 C 运行库）"
    # A target directory of its own keeps these flags from rebuilding the usual one.
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    Invoke-Tool cargo @('build', '--release', '--locked', '--target', $Target,
        '--target-dir', 'target\dist\build')
    $env:RUSTFLAGS = $savedFlags
    $exe = Join-Path $Root "target\dist\build\$Target\release\qqq_monitor.exe"
    $imports = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($exe))
    if ($imports -match '(?i)vcruntime\d+\.dll|msvcp\d+\.dll|api-ms-win-crt-[a-z0-9-]+\.dll') {
        throw "程序仍依赖 C 运行库 $($Matches[0])"
    }

    if (-not $SkipChecks) {
        Write-Step '冒烟测试'
        $log = Join-Path $env:TEMP "qqq_monitor-smoke-$PID"
        $smoke = Start-Process -FilePath $exe -ArgumentList '--smoke-test' -Wait -PassThru `
            -NoNewWindow -RedirectStandardOutput "$log.out" -RedirectStandardError "$log.err"
        Get-Content "$log.out", "$log.err" -Encoding UTF8 | Write-Host
        Remove-Item "$log.out", "$log.err"
        if ($smoke.ExitCode -ne 0) { throw "冒烟测试失败，退出码 $($smoke.ExitCode)" }
    }

    Write-Step "打包到 $output"
    if (Test-Path $output) { Remove-Item $output -Recurse -Force }
    $stage = Join-Path $output $name
    New-Item -ItemType Directory -Path $stage | Out-Null
    Copy-Item $exe, 'LICENSE', 'README.md', 'README.zh-CN.md', 'README.ja.md' $stage
    Copy-Item 'crates\qqq-ui\assets\fonts\LICENSE.txt' (Join-Path $stage 'LICENSE-NotoSansCJK.txt')
    $zip = Join-Path $output "$name.zip"
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::Open($zip, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($file in Get-ChildItem $stage -File) {
            # Entry names use forward slashes, which every unzip tool reads as folders.
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $file.FullName,
                "$name/$($file.Name)", [IO.Compression.CompressionLevel]::Optimal) | Out-Null
        }
    }
    finally {
        $archive.Dispose()
    }
    $sums = Join-Path $output 'SHA256SUMS.txt'
    $hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText($sums, "$hash  $name.zip`n")
    Get-ChildItem $zip, $sums | Format-Table Name, Length -AutoSize | Out-String | Write-Host

    if ($PackageOnly) {
        Write-Step '已完成打包'
        return
    }

    Write-Step "推送 $Branch 与标签 $tag"
    Invoke-Tool git @('push', 'origin', $Branch)
    Invoke-Tool git @('tag', '-a', $tag, '-m', "QQQ Monitor $tag")
    Invoke-Tool git @('push', 'origin', "refs/tags/$tag")

    Write-Step "创建 GitHub Release $tag"
    $release = @('release', 'create', $tag, $zip, $sums, '--title', "QQQ Monitor $tag",
        '--notes-file', $NotesFile, '--verify-tag')
    if ($Draft) { $release += '--draft' }
    Invoke-Tool gh $release
    Write-Step '已完成发布'
}
finally {
    $env:RUSTFLAGS = $savedFlags
    Pop-Location
}
