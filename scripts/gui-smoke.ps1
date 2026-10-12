# Hardware smoke test. Uses the native GUI's public controls, not a test audio path.
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$Device,
    [ValidateRange(1, 8192)]
    [int]$Frames = 512,
    [int]$RunSeconds = 3
)
$ErrorActionPreference = 'Stop'
if ($RunSeconds -lt 1) { throw 'RunSeconds must be positive' }
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class VoiceGuiSmoke {
    public delegate bool Visit(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] public static extern bool EnumThreadWindows(uint thread, Visit visit, IntPtr parameter);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr window, StringBuilder name, int capacity);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr window);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr window, uint msg, IntPtr wp, IntPtr lp);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageTimeoutW")]
    public static extern IntPtr Read(IntPtr window, uint msg, UIntPtr wp, StringBuilder value, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageTimeoutW")]
    public static extern IntPtr Write(IntPtr window, uint msg, UIntPtr wp, string value, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll", EntryPoint="SendMessageTimeoutW")]
    public static extern IntPtr Send(IntPtr window, uint msg, UIntPtr wp, IntPtr lp, uint flags, uint timeout, out UIntPtr result);
}
'@
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExe = Join-Path $taskRoot 'target\release\rust-low-latency-voice-changer-gui.exe'
$taskProcess = Start-Process -FilePath $taskExe -WindowStyle Hidden -PassThru
function Find-ClassWindow([string]$ClassName) {
    $found = [IntPtr]::Zero
    $visit = [VoiceGuiSmoke+Visit]{
        param($handle, $parameter)
        $class = [Text.StringBuilder]::new(256)
        [void][VoiceGuiSmoke]::GetClassNameW($handle, $class, 256)
        if ($class.ToString() -eq $ClassName) { $script:__found = $handle }
        return $true
    }
    $script:__found = [IntPtr]::Zero
    foreach ($thread in $taskProcess.Threads) {
        [void][VoiceGuiSmoke]::EnumThreadWindows($thread.Id, $visit, [IntPtr]::Zero)
    }
    $script:__found
}
function Open-ChildWindow([int]$ButtonId, [string]$ClassName) {
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $window = Find-ClassWindow $ClassName
        if ($window -ne [IntPtr]::Zero) { return $window }
        Click-Button $ButtonId
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Window $ClassName was not created"
}
function Read-Status {
    $taskBuffer = [Text.StringBuilder]::new(8192)
    $taskResult = [UIntPtr]::Zero
    if ([VoiceGuiSmoke]::Read($script:taskStatus, 0xD, [UIntPtr]8192, $taskBuffer, 2, 2000, [ref]$taskResult) -eq [IntPtr]::Zero) {
        throw 'GUI did not respond to status query within 2 seconds'
    }
    $taskBuffer.ToString()
}
function Wait-Status([string]$Pattern) {
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $taskText = Read-Status
        if ($taskText -match $Pattern) { return $taskText }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $taskDeadline)
    throw "Timed out waiting for '$Pattern': $taskText"
}
function Set-Field([int]$Id, [string]$Value) {
    $taskResult = [UIntPtr]::Zero
    $taskControl = [VoiceGuiSmoke]::GetDlgItem($script:taskSettings, $Id)
    if ([VoiceGuiSmoke]::Write($taskControl, 0xC, [UIntPtr]::Zero, $Value, 2, 2000, [ref]$taskResult) -eq [IntPtr]::Zero) {
        throw "Cannot set control $Id"
    }
}
function Click-Button([int]$Id) {
    if (-not [VoiceGuiSmoke]::IsWindowEnabled([VoiceGuiSmoke]::GetDlgItem($script:taskWindow, $Id))) {
        throw "Button $Id is disabled"
    }
    if (-not [VoiceGuiSmoke]::PostMessageW($script:taskWindow, 0x111, [IntPtr]$Id, [IntPtr]::Zero)) {
        throw "Cannot click button $Id"
    }
}
function Wait-Idle {
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(15)
    while (-not [VoiceGuiSmoke]::IsWindowEnabled([VoiceGuiSmoke]::GetDlgItem($script:taskWindow, 101))) {
        if ([DateTime]::UtcNow -gt $taskDeadline) { throw 'GUI did not return to idle' }
        Start-Sleep -Milliseconds 100
    }
}
try {
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $taskProcess.Refresh()
        $script:taskWindow = Find-ClassWindow 'RustLowLatencyVoiceChangerWindow'
        if ($taskWindow -ne [IntPtr]::Zero -and [VoiceGuiSmoke]::GetDlgItem($taskWindow, 101) -ne [IntPtr]::Zero) { break }
        if ($taskProcess.HasExited) { throw 'GUI exited before creating a window' }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $taskDeadline)
    if ($taskWindow -eq [IntPtr]::Zero) { throw 'GUI window was not created' }

    $script:taskSettings = Open-ChildWindow 105 'RustLowLatencyVoiceChangerSettings'
    $script:taskStatusWindow = Open-ChildWindow 106 'RustLowLatencyVoiceChangerStatus'
    $script:taskStatus = [VoiceGuiSmoke]::GetDlgItem($script:taskStatusWindow, 205)
    if ($script:taskStatus -eq [IntPtr]::Zero) { throw 'Status detail control was not created' }

    Write-Output (Wait-Status '^Ready')
    Set-Field 201 $Device
    Set-Field 202 ([string]$Frames)
    Click-Button 103
    Write-Output (Wait-Status '^Inspection OK')
    Wait-Idle
    Set-Field 202 '0'
    Click-Button 101
    Write-Output (Wait-Status '^ERROR: --frames')
    Set-Field 202 ([string]$Frames)
    Set-Field 201 'Missing ASIO smoke device'
    Click-Button 101
    Write-Output (Wait-Status "^ERROR: ASIO device 'Missing ASIO smoke device' not found")
    Wait-Idle
    Set-Field 201 $Device
    foreach ($taskPitch in @('0', '3', '-3')) {
        Set-Field 203 $taskPitch
        Click-Button 101
        $null = Wait-Status '^Running:'
        if ([VoiceGuiSmoke]::IsWindowEnabled([VoiceGuiSmoke]::GetDlgItem($script:taskSettings, 203))) {
            throw 'Settings remained editable during playback'
        }
        Start-Sleep -Seconds $RunSeconds
        Write-Output (Read-Status)
        Click-Button 102
        $taskFinal = Wait-Status '^Stopped:'
        Write-Output $taskFinal
        if ($taskFinal -notmatch 'deadline misses=0' -or $taskFinal -notmatch 'sample-position gaps=0' -or $taskFinal -notmatch 'fault flags=0x0') {
            throw 'Hardware run reported callback faults'
        }
        Wait-Idle
    }
    # Close the window during playback and verify graceful process termination.
    Click-Button 101
    $null = Wait-Status '^Running:'
    [void][VoiceGuiSmoke]::PostMessageW($taskWindow, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
    if (-not $taskProcess.WaitForExit(15000)) { throw 'GUI failed to stop and close within 15 seconds' }
    if ($taskProcess.ExitCode -ne 0) { throw "GUI exited with code $($taskProcess.ExitCode)" }
    Write-Output 'PASS: inspect, validation, worker restart, controls, playback and close while running.'
} finally {
    if (-not $taskProcess.HasExited) {
        [void][VoiceGuiSmoke]::PostMessageW($taskWindow, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
        if (-not $taskProcess.WaitForExit(15000)) {
            Write-Warning 'GUI has not exited; inspect the audio driver before retrying.'
        }
    }
    $taskProcess.Dispose()
}
