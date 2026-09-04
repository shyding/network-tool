param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('focus_window', 'enumerate_controls', 'uia_invoke', 'screenshot')]
    [string]$Action,

    [string]$ControlName,
    [string]$NodeId,
    [string]$ControlType = 'button',
    [string]$OutputPath,
    [string]$ProcessName = '网络测试工具箱',
    [int]$TargetPid = 38080
)

$targetWindow = @{
    processName = $ProcessName
    pid = [string]$TargetPid
}

$actionPayload = @{
    kind = $Action
    targetWindow = $targetWindow
}

if ($Action -eq 'uia_invoke') {
    if ([string]::IsNullOrWhiteSpace($ControlName) -and [string]::IsNullOrWhiteSpace($NodeId)) {
        throw 'ControlName or NodeId is required for uia_invoke.'
    }
    $actionPayload.controlTarget = @{
        name = $ControlName
        nodeId = $NodeId
        controlType = $ControlType
        timeoutMs = 5000
    }
}

$request = @{
    requestId = "ntk-$Action-$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())"
    sessionId = 'network-toolbox-original'
    action = $actionPayload
    policyContext = @{
        allowedRoots = @()
        blockedCapabilities = @()
        operator = 'codex'
        requiresHumanReview = $false
    }
}

$response = Invoke-RestMethod `
    -Method Post `
    -Uri 'http://127.0.0.1:59123/v1/action' `
    -ContentType 'application/json' `
    -Body ($request | ConvertTo-Json -Depth 10)

if ($OutputPath) {
    $parent = Split-Path -Parent $OutputPath
    if ($parent) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $response | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $OutputPath -Encoding utf8
}

$response
