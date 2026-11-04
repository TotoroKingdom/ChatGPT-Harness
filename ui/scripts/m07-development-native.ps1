[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$manifestFile = Join-Path ([IO.Path]::GetTempPath()) ("m07-native-" + [guid]::NewGuid().ToString("N") + ".manifest")

# The Rust test executable does not inherit Tauri's application manifest. Load
# Common Controls v6 explicitly so native Wry tests cannot show a missing
# TaskDialogIndirect dialog. Every test uses disposable app data and fixtures.
$manifest = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0"><dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency></assembly>'
$kitsRoot = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
$mt = Get-ChildItem -LiteralPath $kitsRoot -Recurse -Filter mt.exe -File |
    Where-Object { $_.FullName -match '\\x64\\mt\.exe$' } |
    Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $mt) { throw "Windows SDK manifest tool (x64 mt.exe) is required." }

Push-Location $repositoryRoot
try {
    & cargo build -p chatgpt-harness-runtime --offline
    if ($LASTEXITCODE -ne 0) { throw "Development Runtime build failed." }
    $messages = & cargo test -p chatgpt-harness-ui --lib --features native-tray-tests --no-run --offline --message-format=json
    if ($LASTEXITCODE -ne 0) { throw "Native development test compilation failed." }
    $testExecutable = $messages | ForEach-Object {
        try { $_ | ConvertFrom-Json } catch { $null }
    } | Where-Object {
        $_.reason -eq "compiler-artifact" -and $_.target.name -eq "chatgpt_harness_ui_lib" -and $_.profile.test -and $_.executable
    } | Select-Object -Last 1 -ExpandProperty executable
    if (-not $testExecutable -or -not (Test-Path -LiteralPath $testExecutable -PathType Leaf)) {
        throw "Native development test executable was not reported by Cargo."
    }
    [IO.File]::WriteAllText($manifestFile, $manifest, [Text.UTF8Encoding]::new($false))
    & $mt -nologo -manifest $manifestFile ("-outputresource:$testExecutable;#1")
    if ($LASTEXITCODE -ne 0) { throw "Could not embed the native test manifest." }
    Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public static class M07DevelopmentErrorMode {
    [DllImport("kernel32.dll")] public static extern uint SetErrorMode(uint mode);
}
'@
    $previousErrorMode = [M07DevelopmentErrorMode]::SetErrorMode(0x8003)
    try {
        & $testExecutable --exact desktop::single_instance::tests::duplicate_process_exits_before_host_setup --nocapture
        if ($LASTEXITCODE -ne 0) { throw "Single-instance development test failed." }
        & $testExecutable --exact desktop::autostart_development::isolated_host_development_scenarios --nocapture
        if ($LASTEXITCODE -ne 0) { throw "Auto Start development scenarios failed." }
    } finally {
        [void][M07DevelopmentErrorMode]::SetErrorMode($previousErrorMode)
    }
} finally {
    Pop-Location
    if (Test-Path -LiteralPath $manifestFile) { Remove-Item -LiteralPath $manifestFile -Force }
}
