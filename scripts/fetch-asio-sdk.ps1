# Download the Steinberg ASIO SDK into .tools/asiosdk (must contain common/asio.h).
# Do not commit the SDK; Steinberg's license restricts redistribution.
param(
    [string]$Url = 'https://download.steinberg.net/sdk_downloads/ASIO-SDK_2.3.4_2025-10-15.zip',
    [string]$Destination = ''
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $Destination) {
    $Destination = Join-Path $repoRoot '.tools\asiosdk'
}

$asioHeader = Join-Path $Destination 'common\asio.h'
if (Test-Path -LiteralPath $asioHeader -PathType Leaf) {
    Write-Host "ASIO SDK already present at $Destination"
    exit 0
}

$toolsDir = Split-Path -Parent $Destination
if (-not (Test-Path -LiteralPath $toolsDir)) {
    New-Item -ItemType Directory -Path $toolsDir | Out-Null
}
if (Test-Path -LiteralPath $Destination) {
    Remove-Item -LiteralPath $Destination -Recurse -Force
}

$tempZip = Join-Path ([System.IO.Path]::GetTempPath()) ("asio-sdk-" + [Guid]::NewGuid().ToString('N') + '.zip')
$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("asio-sdk-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempDir | Out-Null
try {
    Write-Host "Downloading ASIO SDK from $Url"
    Invoke-WebRequest -Uri $Url -OutFile $tempZip
    Expand-Archive -LiteralPath $tempZip -DestinationPath $tempDir -Force

    $topLevel = @(Get-ChildItem -LiteralPath $tempDir)
    if ($topLevel.Count -eq 0) {
        throw "ASIO SDK zip from $Url extracted to an empty directory"
    }
    if ($topLevel.Count -eq 1 -and $topLevel[0].PSIsContainer) {
        Move-Item -LiteralPath $topLevel[0].FullName -Destination $Destination
    } else {
        New-Item -ItemType Directory -Path $Destination | Out-Null
        Move-Item -Path (Join-Path $tempDir '*') -Destination $Destination
    }

    if (-not (Test-Path -LiteralPath $asioHeader -PathType Leaf)) {
        throw "ASIO SDK incomplete at ${Destination}: missing common\asio.h"
    }
    Write-Host "ASIO SDK installed at $Destination"
}
finally {
    if (Test-Path -LiteralPath $tempZip) { Remove-Item -LiteralPath $tempZip -Force }
    if (Test-Path -LiteralPath $tempDir) { Remove-Item -LiteralPath $tempDir -Recurse -Force }
}
