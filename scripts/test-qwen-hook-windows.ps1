# Test Qwen Code hook spawn on Windows WITHOUT waiting for a release.
#
# Copies one file to the Windows machine and run in PowerShell:
#   powershell -NoProfile -ExecutionPolicy Bypass -File test-qwen-hook-windows.ps1
#
# What it does:
#   1. Finds ~/.agent-hub/hook-runner/agent-hub-hook.cmd (created when Agent Hub
#      installs any hook). If missing, tells you to open Agent Hub once.
#   2. Replays Qwen's real hookRunner spawn:
#        spawn(cmd.exe, ['/d','/s','/c', command], {shell:false})
#        spawn(powershell, ['-NoProfile','-Command', command], {shell:false})
#      and pipes a fake Qwen UserPromptSubmit JSON to stdin.
#   3. Compares the OLD command (bare quoted .cmd) vs the NEW command
#      (cmd /c "…cmd" --agent-hub-qwen-hook) and whatever is currently in
#      ~/.qwen/settings.json.
#
# Pass = inbox / hook-debug.jsonl / qwen-state.json contains the unique
# session id. Fail = empty stdin, timeout, or the .cmd never starts.
#
# You do NOT need to start Qwen Code. Node is preferred (same spawn as Qwen);
# if node is missing, the script falls back to .NET ProcessStartInfo.

$ErrorActionPreference = 'Stop'

$HomeDir = $env:USERPROFILE
if (-not $HomeDir) { $HomeDir = $env:HOME }
$Runner = Join-Path $HomeDir '.agent-hub\hook-runner\agent-hub-hook.cmd'
$Settings = Join-Path $HomeDir '.qwen\settings.json'
$Monitor = Join-Path $HomeDir '.agent-hub\session-monitor'
$Inbox = Join-Path $Monitor 'inbox'
$DebugLog = Join-Path $Monitor 'hook-debug.jsonl'
$ErrorLog = Join-Path $Monitor 'hook-capture-error.log'
$StateFile = Join-Path $Monitor 'qwen-state.json'
$Arg = '--agent-hub-qwen-hook'

function Write-Head($text) {
    Write-Host ''
    Write-Host "=== $text ===" -ForegroundColor Cyan
}

function Quote-NodeWinArg([string]$arg) {
    # Matches Node/libuv QuoteCmdArg: wrap if space/tab/quote; backslash-escape
    # only the backslashes that sit immediately before a quote.
    if ($arg -notmatch "[ \t`"]") { return $arg }
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append('"')
    $slashes = 0
    foreach ($ch in $arg.ToCharArray()) {
        if ($ch -eq '\') {
            $slashes++
            continue
        }
        if ($ch -eq '"') {
            [void]$sb.Append(('\' * ($slashes * 2 + 1)))
            [void]$sb.Append('"')
            $slashes = 0
            continue
        }
        if ($slashes -gt 0) {
            [void]$sb.Append(('\' * $slashes))
            $slashes = 0
        }
        [void]$sb.Append($ch)
    }
    if ($slashes -gt 0) { [void]$sb.Append(('\' * ($slashes * 2))) }
    [void]$sb.Append('"')
    return $sb.ToString()
}

Write-Head 'Environment'
Write-Host "USERPROFILE  $HomeDir"
Write-Host "runner       $Runner  exists=$([bool](Test-Path $Runner))"
Write-Host "settings     $Settings  exists=$([bool](Test-Path $Settings))"
Write-Host "node         $(if (Get-Command node -ErrorAction SilentlyContinue) { (Get-Command node).Source } else { 'NOT FOUND (using .NET fallback)' })"
Write-Host "ComSpec      $env:ComSpec"

if (-not (Test-Path $Runner)) {
    Write-Host ''
    Write-Host 'hook-runner .cmd is missing. Open Agent Hub on this PC once and' -ForegroundColor Yellow
    Write-Host 'install (or reset) any Hook — that writes the shim. Then re-run.' -ForegroundColor Yellow
    exit 2
}

Write-Host ''
Write-Host '--- hook-runner.cmd ---'
Get-Content -Raw $Runner | Write-Host

$installedCommand = $null
if (Test-Path $Settings) {
    try {
        $json = Get-Content -Raw $Settings | ConvertFrom-Json
        $handlers = @()
        foreach ($event in @('UserPromptSubmit', 'Stop', 'StopFailure')) {
            $groups = $json.hooks.$event
            if (-not $groups) { continue }
            foreach ($group in $groups) {
                foreach ($h in $group.hooks) {
                    if ($h.command -and ($h.command -match 'agent-hub-qwen-hook')) {
                        $handlers += [pscustomobject]@{
                            event   = $event
                            command = [string]$h.command
                            timeout = $h.timeout
                        }
                    }
                }
            }
        }
        Write-Head 'Current ~/.qwen/settings.json (managed handlers)'
        if ($handlers.Count -eq 0) {
            Write-Host 'No Agent Hub Qwen handler found. Install/reset Qwen Hook in Agent Hub first if you want to test the live command.'
        } else {
            $handlers | Format-Table -AutoSize | Out-String | Write-Host
            $installedCommand = $handlers[0].command
            $timeouts = $handlers | Select-Object -ExpandProperty timeout -Unique
            if ($timeouts -contains 10) {
                Write-Host 'WARNING: timeout=10 is 10ms on Qwen (milliseconds). Reset Hook after upgrading.' -ForegroundColor Yellow
            }
        }
        if ($json.disableAllHooks -eq $true) {
            Write-Host 'WARNING: disableAllHooks=true — Qwen will not run any hooks.' -ForegroundColor Yellow
        }
    } catch {
        Write-Host "Could not parse settings.json: $_" -ForegroundColor Yellow
    }
}

$oldCommand = "`"$Runner`" $Arg"
$newCommand = "cmd /c `"$Runner`" $Arg"

$cases = @(
    [pscustomobject]@{ Name = 'direct .cmd (Grok-style, no Qwen wrap)'; Shell = 'direct'; Command = $null }
    [pscustomobject]@{ Name = 'Qwen cmd wrap + OLD quoted path';       Shell = 'cmd';    Command = $oldCommand }
    [pscustomobject]@{ Name = 'Qwen cmd wrap + NEW cmd /c prefix';     Shell = 'cmd';    Command = $newCommand }
    [pscustomobject]@{ Name = 'Qwen powershell wrap + OLD quoted path'; Shell = 'ps';    Command = $oldCommand }
    [pscustomobject]@{ Name = 'Qwen powershell wrap + NEW cmd /c prefix'; Shell = 'ps';  Command = $newCommand }
)
if ($installedCommand) {
    $cases += [pscustomobject]@{
        Name    = 'Qwen cmd wrap + LIVE settings.json command'
        Shell   = 'cmd'
        Command = $installedCommand
    }
}

$nodeScript = @'
const { spawn } = require("child_process");
const fs = require("fs");

const spec = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const payload = fs.readFileSync(process.argv[3], "utf8");

let exe, args;
if (spec.shell === "direct") {
  exe = spec.runner;
  args = [spec.arg];
} else if (spec.shell === "cmd") {
  exe = process.env.ComSpec || "cmd.exe";
  args = ["/d", "/s", "/c", spec.command];
} else if (spec.shell === "ps") {
  exe = "powershell.exe";
  args = ["-NoProfile", "-Command", spec.command];
} else {
  process.stderr.write("unknown shell\n");
  process.exit(2);
}

const child = spawn(exe, args, {
  env: { ...process.env, AGENT_HUB_HOOK_DEBUG: "1" },
  stdio: ["pipe", "pipe", "pipe"],
  shell: false,
  windowsHide: true,
});

let stdout = "";
let stderr = "";
child.stdout.on("data", (d) => { stdout += d.toString(); });
child.stderr.on("data", (d) => { stderr += d.toString(); });

const timer = setTimeout(() => {
  try { child.kill(); } catch {}
  finish(124, "timeout");
}, spec.timeoutMs || 15000);

let finished = false;
function finish(code, extra) {
  if (finished) return;
  finished = true;
  clearTimeout(timer);
  process.stdout.write(JSON.stringify({
    code: code == null ? 1 : code,
    extra: extra || null,
    stdout,
    stderr,
  }));
}

child.on("error", (err) => finish(1, String(err)));
child.on("close", (code) => finish(code, null));

try {
  child.stdin.write(payload);
  child.stdin.end();
} catch (err) {
  // EPIPE: child exited before stdin completed.
}
'@

function Invoke-HookCase($case, $sessionId) {
    $payloadObj = @{
        session_id      = $sessionId
        hook_event_name = 'UserPromptSubmit'
        cwd             = $HomeDir
        prompt          = "agent-hub qwen hook probe $sessionId"
        timestamp       = (Get-Date).ToString('o')
    }
    $payload = $payloadObj | ConvertTo-Json -Compress

    $spec = @{
        shell     = $case.Shell
        command   = $case.Command
        runner    = $Runner
        arg       = $Arg
        timeoutMs = 15000
    } | ConvertTo-Json -Compress

    $beforeDebug = if (Test-Path $DebugLog) { (Get-Item $DebugLog).Length } else { 0 }
    $beforeErr = if (Test-Path $ErrorLog) { (Get-Item $ErrorLog).Length } else { 0 }

    $result = $null
    $node = Get-Command node -ErrorAction SilentlyContinue
    if ($node) {
        $work = Join-Path $env:TEMP 'agent-hub-qwen-hook-probe'
        New-Item -ItemType Directory -Force -Path $work | Out-Null
        $js = Join-Path $work 'probe.js'
        $specFile = Join-Path $work 'spec.json'
        $payloadFile = Join-Path $work 'payload.json'
        $utf8 = New-Object System.Text.UTF8Encoding $false
        [System.IO.File]::WriteAllText($js, $nodeScript, $utf8)
        [System.IO.File]::WriteAllText($specFile, $spec, $utf8)
        [System.IO.File]::WriteAllText($payloadFile, $payload, $utf8)
        $out = & node $js $specFile $payloadFile 2>&1 | Out-String
        try { $result = $out.Trim() | ConvertFrom-Json } catch {
            $result = [pscustomobject]@{ code = 1; extra = "node output not JSON: $out"; stdout = ''; stderr = $out }
        }
    } else {
        $result = Invoke-HookCaseDotNet $case $payload
    }

    Start-Sleep -Milliseconds 400

    $hitDebug = $false
    $hitInbox = $false
    $hitState = $false
    $errTail = ''
    if (Test-Path $DebugLog) {
        $hitDebug = [bool](Select-String -Path $DebugLog -SimpleMatch $sessionId -ErrorAction SilentlyContinue)
    }
    if (Test-Path $Inbox) {
        $hitInbox = [bool](Get-ChildItem $Inbox -File -ErrorAction SilentlyContinue |
            Where-Object {
                $raw = Get-Content $_.FullName -Raw -ErrorAction SilentlyContinue
                $raw -and $raw.Contains($sessionId)
            })
    }
    if (Test-Path $StateFile) {
        $stateRaw = Get-Content $StateFile -Raw -ErrorAction SilentlyContinue
        $hitState = [bool]($stateRaw -and $stateRaw.Contains($sessionId))
    }
    if (Test-Path $ErrorLog) {
        $afterErr = (Get-Item $ErrorLog).Length
        if ($afterErr -gt $beforeErr) {
            $errTail = Get-Content $ErrorLog -Tail 5 | Out-String
        }
    }

    $ok = [bool]($hitDebug -or $hitInbox -or $hitState)
    [pscustomobject]@{
        Name     = $case.Name
        Command  = $(if ($case.Command) { $case.Command } else { "$Runner $Arg" })
        Exit     = $result.code
        Extra    = $result.extra
        Captured = $ok
        Debug    = [bool]$hitDebug
        Inbox    = [bool]$hitInbox
        State    = [bool]$hitState
        Stderr   = ($(if ($result.stderr) { $result.stderr.Trim() } else { '' }))
        ErrLog   = $errTail.Trim()
    }
}

function Invoke-HookCaseDotNet($case, $payload) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $psi.EnvironmentVariables['AGENT_HUB_HOOK_DEBUG'] = '1'

    if ($case.Shell -eq 'direct') {
        $psi.FileName = $Runner
        $psi.Arguments = $Arg
    } elseif ($case.Shell -eq 'cmd') {
        $psi.FileName = $(if ($env:ComSpec) { $env:ComSpec } else { 'cmd.exe' })
        $quoted = Quote-NodeWinArg $case.Command
        $psi.Arguments = "/d /s /c $quoted"
    } else {
        $psi.FileName = 'powershell.exe'
        $quoted = Quote-NodeWinArg $case.Command
        $psi.Arguments = "-NoProfile -Command $quoted"
    }

    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $psi
    [void]$p.Start()
    $p.StandardInput.Write($payload)
    $p.StandardInput.Close()
    if (-not $p.WaitForExit(15000)) {
        try { $p.Kill() } catch {}
        return [pscustomobject]@{ code = 124; extra = 'timeout'; stdout = ''; stderr = '' }
    }
    [pscustomobject]@{
        code   = $p.ExitCode
        extra  = $null
        stdout = $p.StandardOutput.ReadToEnd()
        stderr = $p.StandardError.ReadToEnd()
    }
}

Write-Head 'Running probes'
New-Item -ItemType Directory -Force -Path $Inbox | Out-Null
$env:AGENT_HUB_HOOK_DEBUG = '1'

$rows = @()
foreach ($case in $cases) {
    $sid = "probe-$(Get-Date -Format 'HHmmss')-$([guid]::NewGuid().ToString('N').Substring(0,8))"
    Write-Host "-> $($case.Name)"
    $row = Invoke-HookCase $case $sid
    $color = if ($row.Captured) { 'Green' } else { 'Red' }
    Write-Host ("   captured={0} exit={1} extra={2}" -f $row.Captured, $row.Exit, $row.Extra) -ForegroundColor $color
    if ($row.Stderr) { Write-Host "   stderr: $($row.Stderr.Substring(0, [Math]::Min(300, $row.Stderr.Length)))" }
    if ($row.ErrLog) { Write-Host "   error.log: $($row.ErrLog)" -ForegroundColor Yellow }
    $rows += $row
}

Write-Head 'Summary'
$rows | Select-Object Name, Captured, Exit, Extra | Format-Table -AutoSize | Out-String | Write-Host

$oldCmd = $rows | Where-Object { $_.Name -like '*OLD*' }
$newCmd = $rows | Where-Object { $_.Name -like '*NEW*' }
$oldAny = @($oldCmd | Where-Object { $_.Captured }).Count -gt 0
$newAll = @($newCmd).Count -gt 0 -and (@($newCmd | Where-Object { -not $_.Captured }).Count -eq 0)
$newAny = @($newCmd | Where-Object { $_.Captured }).Count -gt 0
$direct = $rows | Where-Object { $_.Name -like 'direct*' } | Select-Object -First 1

Write-Head 'How to read this'
if ($direct -and -not $direct.Captured) {
    Write-Host 'Direct .cmd spawn already failed. Agent Hub exe/shim is not receiving stdin.' -ForegroundColor Red
    Write-Host 'Open Agent Hub, reset any Hook so hook-runner.cmd is rewritten, then retry.'
    exit 1
}
if ($newAll) {
    Write-Host 'NEW command (cmd /c prefix) CAN be triggered under Qwen''s spawn. Safe to ship that form.' -ForegroundColor Green
} elseif ($newAny) {
    Write-Host 'NEW command worked on some shells but not all. Paste the Summary table before shipping.' -ForegroundColor Yellow
} else {
    Write-Host 'NEW command did NOT capture. Do not ship yet — hook still cannot start under Qwen wrap.' -ForegroundColor Red
}
if ($oldAny) {
    Write-Host 'OLD quoted-path command also worked here. Unexpected, but not a blocker.'
} else {
    Write-Host 'OLD quoted-path command failed (expected on real Qwen Windows). Reset Hook after upgrade.'
}
if ($installedCommand -and $installedCommand -notmatch '^\s*cmd /c ') {
    Write-Host ''
    Write-Host 'Live settings.json is still the OLD command. Even after this probe passes,' -ForegroundColor Yellow
    Write-Host 'Qwen will keep using the dead command until you Reset Qwen Hook in Agent Hub.'
}
