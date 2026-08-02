[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$version = "3.13.0"
$platformKey = "$($env:RUNNER_OS)-$($env:RUNNER_ARCH)"
$packages = @{
    "Linux-X64" = @{
        Asset = "protoc-3.13.0-linux-x86_64.zip"
        Sha256 = "4a3b26d1ebb9c1d23e933694a6669295f6a39ddc64c3db2adf671f0a6026f82e"
        Executable = "protoc"
    }
    "macOS-X64" = @{
        Asset = "protoc-3.13.0-osx-x86_64.zip"
        Sha256 = "a201954cc7d1a309b5f4feacd23a0abcf3ffc20eb15e79c9a0856a5804f6c34c"
        Executable = "protoc"
    }
    "Windows-X64" = @{
        Asset = "protoc-3.13.0-win64.zip"
        Sha256 = "326a18c917cce8bc58fa6741260f6fb733186ffdab728a952b4cf31e57a76b91"
        Executable = "protoc.exe"
    }
}

if (-not $packages.ContainsKey($platformKey)) {
    throw "protoc $version is not configured for runner platform $platformKey"
}
if ([string]::IsNullOrWhiteSpace($env:RUNNER_TOOL_CACHE)) {
    throw "RUNNER_TOOL_CACHE is not defined"
}
if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
    throw "RUNNER_TEMP is not defined"
}
if ([string]::IsNullOrWhiteSpace($env:GITHUB_PATH)) {
    throw "GITHUB_PATH is not defined"
}
if ([string]::IsNullOrWhiteSpace($env:GITHUB_OUTPUT)) {
    throw "GITHUB_OUTPUT is not defined"
}

$package = $packages[$platformKey]
$toolRoot = Join-Path $env:RUNNER_TOOL_CACHE "ym-connect-protoc/$version/$platformKey"
$binPath = Join-Path $toolRoot "bin"
$executablePath = Join-Path $binPath $package.Executable

if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
    $archivePath = Join-Path $env:RUNNER_TEMP $package.Asset
    $extractPath = Join-Path $env:RUNNER_TEMP "ym-connect-protoc-$platformKey"
    $downloadUrls = @(
        "https://github.com/protocolbuffers/protobuf/releases/download/v$version/$($package.Asset)",
        (
            "https://mirror.bazel.build/github.com/protocolbuffers/protobuf/releases/" +
            "download/v$version/$($package.Asset)"
        )
    )

    Remove-Item -LiteralPath $archivePath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $extractPath -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $toolRoot -Recurse -Force -ErrorAction SilentlyContinue

    $downloaded = $false
    foreach ($url in $downloadUrls) {
        try {
            Remove-Item -LiteralPath $archivePath -Force -ErrorAction SilentlyContinue
            Invoke-WebRequest -Uri $url -OutFile $archivePath
            $downloaded = $true
            break
        }
        catch {
            Write-Warning "Failed to download $url`: $($_.Exception.Message)"
        }
    }
    if (-not $downloaded) {
        throw "Unable to download $($package.Asset) from the configured release sources"
    }

    $actualSha256 = (
        Get-FileHash -LiteralPath $archivePath -Algorithm SHA256
    ).Hash.ToLowerInvariant()
    if ($actualSha256 -ne $package.Sha256) {
        throw (
            "Checksum mismatch for $($package.Asset): " +
            "expected $($package.Sha256), got $actualSha256"
        )
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractPath
    New-Item -ItemType Directory -Path (Split-Path -Parent $toolRoot) -Force | Out-Null
    Move-Item -LiteralPath $extractPath -Destination $toolRoot
    Remove-Item -LiteralPath $archivePath -Force

    if ($env:RUNNER_OS -ne "Windows") {
        & chmod +x $executablePath
        if ($LASTEXITCODE -ne 0) {
            throw "chmod failed for $executablePath"
        }
    }
}

$reportedVersion = (& $executablePath --version).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "protoc failed to report its version"
}
if ($reportedVersion -ne "libprotoc $version") {
    throw "Unexpected protoc version: $reportedVersion"
}

$binPath | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
"path=$toolRoot" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
"version=$version" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
Write-Host "Installed $reportedVersion from verified archive $($package.Asset)"
