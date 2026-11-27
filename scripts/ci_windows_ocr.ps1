# Provision only the ephemeral hosted runner. WinRT projections need Windows PowerShell.
param([Parameter(Mandatory=$true)][ValidateSet('en-US','ko-KR')][string]$Language)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'OCR capability provisioning requires an ephemeral GitHub-hosted Windows runner.'
}
$dir = Join-Path $env:RUNNER_TEMP 'clipcat-windows-ocr'
New-Item -Force -ItemType Directory $dir | Out-Null
Start-Transcript -Path (Join-Path $dir "$Language-profile.txt") -Force
$iso = Join-Path $env:RUNNER_TEMP 'clipcat-server-2025-languages.iso'
$mounted = $false
try {
    $capabilities = @("Language.OCR~~~$Language~0.0.1.0")
    if ($Language -eq 'ko-KR') {
        $capabilities = @("Language.Basic~~~ko-KR~0.0.1.0", "Language.Fonts.Kore~~~und-KORE~0.0.1.0") + $capabilities
    }
    $source = @{}
    # Windows Update acquisition can stall on hosted Server images. Use
    # Microsoft's matching language/optional-feature repository instead.
    $missing = @($capabilities | Where-Object { (Get-WindowsCapability -Online -Name $_).State -ne 'Installed' })
    if ($missing.Count -gt 0) {
        if ([Environment]::OSVersion.Version.Build -ne 26100) { throw 'Runner needs matching language media for its OS build.' }
        Write-Host "Missing capabilities: $($missing -join ', '); acquiring official Server 2025 media"
        & curl.exe --fail --location --silent --show-error --max-time 900 --output $iso 'https://go.microsoft.com/fwlink/?linkid=2273506'
        if ($LASTEXITCODE -ne 0) { throw 'Language media download failed.' }
        $disk = Mount-DiskImage -ImagePath $iso -Access ReadOnly -PassThru
        $mounted = $true
        $volume = $disk | Get-Volume
        $repo = "$($volume.DriveLetter):\LanguagesAndOptionalFeatures"
        if (!(Test-Path $repo)) { throw "Language repository missing: $repo" }
        $source = @{Source=$repo; LimitAccess=$true}
    }
    foreach ($name in $capabilities) {
        $item = Get-WindowsCapability -Online -Name $name
        if ($item.State -ne 'Installed') {
            Write-Host "Installing $name"
            Add-WindowsCapability -Online -Name $name @source | Out-Host
        }
        Get-WindowsCapability -Online -Name $name | Format-List Name, State | Out-Host
    }
    Set-WinUserLanguageList (New-WinUserLanguageList $Language) -Force
    $profileTag = (Get-WinUserLanguageList)[0].LanguageTag
    Write-Host "First user-profile language: $profileTag"
    $requiredLanguage = [Globalization.CultureInfo]::GetCultureInfo($Language).TwoLetterISOLanguageName
    if ([Globalization.CultureInfo]::GetCultureInfo($profileTag).TwoLetterISOLanguageName -ne $requiredLanguage) {
        throw "Unexpected user-profile language: $profileTag"
    }
    # Probe in a fresh process, just as the app starts a fresh worker.
    $env:CLIPCAT_CI_LANGUAGE = $requiredLanguage
    $probe = @'
$ErrorActionPreference = 'Stop'
[Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime] | Out-Null
'Installed recognizers: ' + ([Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages.LanguageTag -join ', ')
$engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
'Profile recognizer: ' + $engine.RecognizerLanguage.LanguageTag
if (!$engine -or !$engine.RecognizerLanguage.LanguageTag) { exit 2 }
# Windows normalizes Korean to the neutral BCP-47 tag ko on this image.
$selected = [Globalization.CultureInfo]::GetCultureInfo($engine.RecognizerLanguage.LanguageTag)
if ($selected.TwoLetterISOLanguageName -ne $env:CLIPCAT_CI_LANGUAGE) { exit 1 }
'@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($probe))
    $result = & powershell.exe -NoProfile -NonInteractive -EncodedCommand $encoded
    $code = $LASTEXITCODE
    $result | Tee-Object -FilePath (Join-Path $dir "$Language-recognizer.txt") | Out-Host
    if ($code -ne 0) { throw "No matching $Language profile recognizer (probe exit $code)." }
} finally {
    if ($mounted) { Dismount-DiskImage -ImagePath $iso }
    if (Test-Path $iso) { Remove-Item -LiteralPath $iso }
    Stop-Transcript
}
