$BUILD_TOOLS = 'C:\Programs\BuildTools'

if (-not (Test-Path $BUILD_TOOLS)) {
  Write-Host "BUILD_TOOLS not found at: $BUILD_TOOLS"
  exit 1
}

& "$BUILD_TOOLS\devcmd.ps1"
