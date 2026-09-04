param(
    [switch]$Force,
    [switch]$SkipInstaller,
    [switch]$FingerprintOnly,
    [switch]$ReuseCompiled
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cacheDir = Join-Path $projectRoot '.build-cache'
$distDir = Join-Path $projectRoot 'dist\windows-x64'
$stampFile = Join-Path $cacheDir 'windows-x64.sha256'
$compiledStampFile = Join-Path $cacheDir 'windows-x64-release.sha256'

function Get-SourceFingerprint {
    $roots = @(
        (Join-Path $projectRoot 'src-tauri\src'),
        (Join-Path $projectRoot 'src-tauri\icons'),
        (Join-Path $projectRoot 'analysis\tauri-assets')
    )
    $files = @(
        (Join-Path $projectRoot 'src-tauri\Cargo.toml'),
        (Join-Path $projectRoot 'src-tauri\Cargo.lock'),
        (Join-Path $projectRoot 'src-tauri\build.rs'),
        (Join-Path $projectRoot 'src-tauri\tauri.conf.json')
    ) + @(Get-ChildItem -LiteralPath $roots -File -Recurse | Select-Object -ExpandProperty FullName)
    $lines = foreach ($file in ($files | Sort-Object -Unique)) {
        '{0}  {1}' -f (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash, $file
    }
    $bytes = [Text.Encoding]::UTF8.GetBytes(($lines -join "`n"))
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        -join ($sha.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') })
    } finally {
        $sha.Dispose()
    }
}

New-Item -ItemType Directory -Force -Path $cacheDir, $distDir | Out-Null
$fingerprint = Get-SourceFingerprint
if ($FingerprintOnly) {
    Write-Output $fingerprint
    exit 0
}
$cached = if (Test-Path -LiteralPath $stampFile) { (Get-Content -LiteralPath $stampFile -Raw).Trim() } else { '' }
$existing = @(Get-ChildItem -LiteralPath $distDir -File -ErrorAction SilentlyContinue)
if (-not $Force -and $fingerprint -eq $cached -and $existing.Count -gt 0) {
    Write-Host "源码未变化，复用 Windows 产物：$distDir"
    $existing | Select-Object Name, Length, LastWriteTime
    exit 0
}

if (-not $SkipInstaller -and -not (Get-Command cargo-tauri -ErrorAction SilentlyContinue)) {
    Write-Host '首次生成安装包：安装 tauri-cli（以后将直接复用）...'
    cargo +1.98.0 install tauri-cli --version '^2' --locked
    if ($LASTEXITCODE -ne 0) { throw "tauri-cli 安装失败，退出码 $LASTEXITCODE" }
}

Push-Location (Join-Path $projectRoot 'src-tauri')
try {
    if ($ReuseCompiled) {
        $compiledExe = Join-Path $projectRoot 'src-tauri\target\release\network-toolbox-rebuild.exe'
        if (-not (Test-Path -LiteralPath $compiledExe)) {
            throw 'No compiled release executable is available to reuse.'
        }
        $compiledFingerprint = if (Test-Path -LiteralPath $compiledStampFile) { (Get-Content -LiteralPath $compiledStampFile -Raw).Trim() } else { '' }
        if ($compiledFingerprint -ne $fingerprint) {
            throw 'The compiled release does not match the current source. Run once without -ReuseCompiled.'
        }
        Write-Host 'Reusing the existing compiled release executable.'
    } elseif ($SkipInstaller) {
        cargo +1.98.0 build --release --locked
    } else {
        cargo +1.98.0 tauri build --bundles nsis
    }
    if (-not $ReuseCompiled -and $LASTEXITCODE -ne 0) {
        throw "Windows build failed with exit code $LASTEXITCODE"
    }
    if (-not $ReuseCompiled) {
        Set-Content -LiteralPath $compiledStampFile -Value $fingerprint -NoNewline
    }
} finally {
    Pop-Location
}

$releaseDir = Join-Path $projectRoot 'src-tauri\target\release'
$portableName = (-join @(
    [char]0x7F51, [char]0x7EDC, [char]0x6D4B, [char]0x8BD5,
    [char]0x5DE5, [char]0x5177, [char]0x7BB1,
    '_9.0.0_x64_',
    [char]0x4FBF, [char]0x643A, [char]0x7248,
    '.exe'
))
$compiledPortable = Join-Path $releaseDir 'network-toolbox-rebuild.exe'
$portablePath = Join-Path $distDir $portableName
try {
    Copy-Item -LiteralPath $compiledPortable -Destination $portablePath -Force
} catch [System.IO.IOException] {
    $fingerprintSuffix = $fingerprint.Substring(0, 12)
    $fallbackName = [IO.Path]::GetFileNameWithoutExtension($portableName) + '_' + $fingerprintSuffix + '.exe'
    $portablePath = Join-Path $distDir $fallbackName
    Copy-Item -LiteralPath $compiledPortable -Destination $portablePath -Force
    Write-Host "Canonical portable executable is in use; wrote: $portablePath"
}
if (-not $SkipInstaller) {
    Get-ChildItem -LiteralPath (Join-Path $releaseDir 'bundle\nsis') -Filter '*.exe' -File |
        Copy-Item -Destination $distDir -Force
}
Set-Content -LiteralPath $stampFile -Value $fingerprint -NoNewline
Write-Host "Windows 产物已生成：$distDir"
Write-Output $portablePath
