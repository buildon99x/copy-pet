# Provision only the ephemeral hosted runner. WinRT projections need Windows PowerShell.
param([Parameter(Mandatory=$true)][ValidateSet('en-US','ko-KR')][string]$Language)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'OCR capability provisioning requires an ephemeral GitHub-hosted Windows runner.'
}
$dir = Join-Path $env:RUNNER_TEMP 'clipcat-windows-ocr'
New-Item -Force -ItemType Directory $dir | Out-Null
Start-Transcript -Path (Join-Path $dir "$Language-profile.txt") -Force
try {
    $capabilities = @("Language.OCR~~~$Language~0.0.1.0")
    if ($Language -eq 'ko-KR') {
        $capabilities = @("Language.Basic~~~ko-KR~0.0.1.0", "Language.Fonts.Kore~~~und-KORE~0.0.1.0") + $capabilities
    }
    foreach ($name in $capabilities) {
        $item = Get-WindowsCapability -Online -Name $name
        if ($item.State -ne 'Installed') {
            Add-WindowsCapability -Online -Name $name | Out-Host
        }
        Get-WindowsCapability -Online -Name $name | Format-List Name, State | Out-Host
    }
    Set-WinUserLanguageList (New-WinUserLanguageList $Language) -Force
    Get-WinUserLanguageList | Format-List LanguageTag | Out-Host
    # Probe in a fresh process, just as the app starts a fresh worker.
    $env:CLIPCAT_CI_PROFILE = $Language
    $probe = @'
$ErrorActionPreference = 'Stop'
[Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime] | Out-Null
'Installed recognizers: ' + ([Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages.LanguageTag -join ', ')
$engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
'Profile recognizer: ' + $engine.RecognizerLanguage.LanguageTag
if ($engine.RecognizerLanguage.LanguageTag -ne $env:CLIPCAT_CI_PROFILE) { exit 1 }
'@
    & powershell.exe -NoProfile -NonInteractive -Command $probe
    if ($LASTEXITCODE -ne 0) { throw "No matching $Language profile recognizer." }
} finally {
    Stop-Transcript
}
