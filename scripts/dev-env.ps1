# Dot-source from PowerShell: . .\scripts\dev-env.ps1
# Session-only paths for the toolchain installed during Phase 1 verification.
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskCargo = Join-Path $taskRoot '.tools\cargo'
$taskRustup = Join-Path $taskRoot '.tools\rustup'
$taskClang = Join-Path $taskRoot '.tools\python\clang\native'
$taskAsio = Join-Path $taskRoot '.tools\asio-sdk'
foreach ($taskPath in @((Join-Path $taskCargo 'bin\cargo.exe'), $taskRustup, (Join-Path $taskClang 'libclang.dll'), (Join-Path $taskAsio 'common\asio.h'))) {
    if (-not (Test-Path -LiteralPath $taskPath)) {
        throw "Local build dependency missing: $taskPath. See README.md for external toolchain setup."
    }
}
$env:CARGO_HOME = $taskCargo
$env:RUSTUP_HOME = $taskRustup
$env:LIBCLANG_PATH = $taskClang
$env:CPAL_ASIO_DIR = $taskAsio
$env:Path = (Join-Path $taskCargo 'bin') + [IO.Path]::PathSeparator + $env:Path
