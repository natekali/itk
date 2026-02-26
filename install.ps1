#Requires -Version 5.1
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo        = "natekali/itk"
$AssetName   = "itk-windows-x86_64.exe"
$InstallDir  = "$env:ProgramFiles\itk"
$BinPath     = "$InstallDir\itk.exe"
$BaseUrl     = "https://github.com/$Repo/releases/latest/download"
$DownloadUrl = "$BaseUrl/$AssetName"

Write-Host "Downloading itk from $DownloadUrl ..."

# ── create install directory ──────────────────────────────────────────────────
if (-not (Test-Path $InstallDir)) {
    try {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    } catch {
        Write-Error "Failed to create '$InstallDir'. Try running as Administrator."
        exit 1
    }
}

# ── download binary ───────────────────────────────────────────────────────────
$TempFile = [System.IO.Path]::GetTempFileName() + ".exe"
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempFile -UseBasicParsing
} catch {
    Write-Error "Download failed: $_"
    Remove-Item -Path $TempFile -Force -ErrorAction SilentlyContinue
    exit 1
}

# ── install binary ────────────────────────────────────────────────────────────
try {
    Move-Item -Path $TempFile -Destination $BinPath -Force
} catch {
    Write-Error "Failed to install to '$BinPath'. Try running as Administrator."
    Remove-Item -Path $TempFile -Force -ErrorAction SilentlyContinue
    exit 1
}

# ── add to PATH ───────────────────────────────────────────────────────────────
$MachineScope = $false
$CurrentMachinePath = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
if ($CurrentMachinePath -notlike "*$InstallDir*") {
    try {
        [System.Environment]::SetEnvironmentVariable(
            "Path",
            "$CurrentMachinePath;$InstallDir",
            "Machine"
        )
        $MachineScope = $true
        Write-Host "Added '$InstallDir' to system PATH."
    } catch {
        # Fallback: user-level PATH
        $UserPath = [System.Environment]::GetEnvironmentVariable("Path", "User")
        if ($UserPath -notlike "*$InstallDir*") {
            [System.Environment]::SetEnvironmentVariable(
                "Path",
                "$UserPath;$InstallDir",
                "User"
            )
            Write-Host "Added '$InstallDir' to user PATH (restart terminal to apply)."
        }
    }
}

# ── verify ────────────────────────────────────────────────────────────────────
Write-Host ""
if (Test-Path $BinPath) {
    Write-Host "v  itk installed to $BinPath"
    Write-Host ""
    Write-Host "Quick start:"
    Write-Host "  Copy a stack trace -> run: itk"
    Write-Host "  Pipe a git diff:           git diff | itk --diff"
    Write-Host "  See token savings:         itk gain"
    Write-Host ""
    Write-Host "Note: Restart your terminal for PATH changes to take effect."
} else {
    Write-Error "Installation may have failed - binary not found at $BinPath"
    exit 1
}
