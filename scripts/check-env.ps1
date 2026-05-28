$ErrorActionPreference = "Stop"

function Show-CommandVersion {
  param(
    [Parameter(Mandatory = $true)] [string] $Name,
    [Parameter(Mandatory = $true)] [string] $Command
  )

  try {
    $version = Invoke-Expression $Command
    Write-Host "[OK] ${Name}: $version" -ForegroundColor Green
  } catch {
    Write-Host "[MISS] ${Name}" -ForegroundColor Red
    throw
  }
}

Write-Host "Hi-Voicer environment check" -ForegroundColor Cyan

Show-CommandVersion "Node.js" "node --version"
Show-CommandVersion "npm" "npm --version"
Show-CommandVersion "rustc" "rustc --version"
Show-CommandVersion "cargo" "cargo --version"

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
  $vsPath = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  if ($vsPath) {
    Write-Host "[OK] Visual Studio C++ Build Tools: $vsPath" -ForegroundColor Green
  } else {
    throw "Missing Visual Studio C++ Build Tools."
  }
} else {
  throw "Missing vswhere.exe. Install Microsoft C++ Build Tools."
}

$webview = Get-ItemProperty HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\* -ErrorAction SilentlyContinue |
  Where-Object { $_.name -like "*WebView2*" } |
  Select-Object -First 1

if ($webview) {
  Write-Host "[OK] Microsoft Edge WebView2 Runtime: $($webview.pv)" -ForegroundColor Green
} else {
  throw "Missing Microsoft Edge WebView2 Runtime."
}

Write-Host "[OK] Environment check completed" -ForegroundColor Green
