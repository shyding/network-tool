$ErrorActionPreference = 'Stop'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9222'
$root = Join-Path $PSScriptRoot 'original-extracted'
$exe = Get-ChildItem -Recurse -File -LiteralPath $root |
    Where-Object { $_.Length -eq 24212480 } |
    Select-Object -ExpandProperty FullName -First 1

if (-not $exe) {
    throw 'Original main executable was not found.'
}

$process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -PassThru
Start-Sleep -Seconds 8
$process.Refresh()
[PSCustomObject]@{
    Id = $process.Id
    Title = $process.MainWindowTitle
    Handle = $process.MainWindowHandle
}
