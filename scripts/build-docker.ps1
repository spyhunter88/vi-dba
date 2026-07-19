# Build Linux binaries (.deb and .AppImage) using Docker (PowerShell version).
# This is designed for Windows users running PowerShell.

$RootDir = (Resolve-Path "$PSScriptRoot\..").Path
$ImageName = "vidba-linux-builder"

Write-Host "==> Building Docker image: $ImageName" -ForegroundColor Cyan
docker build -t $ImageName -f "$RootDir\Dockerfile.linux" $RootDir

if ($LASTEXITCODE -ne 0) {
    Write-Error "Docker build failed"
    exit 1
}

Write-Host "==> Running build inside Docker container" -ForegroundColor Cyan
# Mount the root directory to /workspace, but use anonymous volumes to isolate:
# - node_modules: prevents overwriting host's Windows node_modules
# - target: prevents Rust compilation target conflicts
# The compiled outputs will still be written to /workspace/dist, which maps back to the host.
# We pass $args to forward parameters like features.
docker run --rm `
  -v "${RootDir}:/workspace" `
  -v "/workspace/frontend/node_modules" `
  -v "/workspace/src-tauri/target" `
  -v "/workspace/target" `
  $ImageName $args

if ($LASTEXITCODE -ne 0) {
    Write-Error "Containerized build failed"
    exit 1
}

Write-Host "==> Build process completed! Check the 'dist' directory for outputs." -ForegroundColor Green
if (Test-Path "$RootDir\dist") {
    Get-ChildItem "$RootDir\dist"
}
