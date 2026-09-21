[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$uiDirectory = (Resolve-Path (Join-Path $scriptDirectory "..")).Path
$source = Join-Path $uiDirectory "src-tauri\icons\portable\portable-icon-master.png"
$output = Join-Path $uiDirectory "src-tauri\icons\portable\generated"
$sizes = @(16, 32, 48, 64, 128, 256)

if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
    throw "Portable icon master is missing: $source"
}
New-Item -ItemType Directory -Force -Path $output | Out-Null

Push-Location $uiDirectory
try {
    & npx tauri icon $source --output $output --png 16 --png 32 --png 48 --png 64 --png 128 --png 256
    if ($LASTEXITCODE -ne 0) { throw "Portable PNG icon generation failed." }
} finally {
    Pop-Location
}

$images = @($sizes | ForEach-Object {
    $path = Join-Path $output "${_}x${_}.png"
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Generated icon is missing: $path" }
    [pscustomobject]@{ Size = $_; Bytes = [IO.File]::ReadAllBytes($path) }
})
$iconPath = Join-Path $output "icon.ico"
$stream = [IO.File]::Open($iconPath, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::None)
$writer = [IO.BinaryWriter]::new($stream)
try {
    $writer.Write([uint16]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]$images.Count)
    $offset = 6 + (16 * $images.Count)
    foreach ($image in $images) {
        $dimension = if ($image.Size -eq 256) { 0 } else { $image.Size }
        $writer.Write([byte]$dimension)
        $writer.Write([byte]$dimension)
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$image.Bytes.Length)
        $writer.Write([uint32]$offset)
        $offset += $image.Bytes.Length
    }
    foreach ($image in $images) { $writer.Write([byte[]]$image.Bytes) }
} finally {
    $writer.Dispose()
    $stream.Dispose()
}

Write-Output "Generated Portable icons: $output"
