# Runs only on a disposable CI desktop. This launches the staged Store binary;
# it does not claim an MSIX deployment or Windows App Certification Kit test.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ClipCatSmoke {
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
}
'@
$out = Join-Path $PWD 'target/store/evidence'
New-Item -ItemType Directory -Force $out | Out-Null
$testData = Join-Path $out 'profile'
New-Item -ItemType Directory -Force (Join-Path $testData 'ClipCat') | Out-Null
$previousAppData = $env:APPDATA
$env:APPDATA = $testData
$app = $null
try {
    @{ lang = 'en'; onboarded = $true; auto_update = $true } | ConvertTo-Json | Set-Content (Join-Path $testData 'ClipCat/state.json')
    $app = Start-Process (Join-Path $PWD 'target/store/stage/clipcat.exe') -PassThru
    Start-Sleep -Seconds 4
    if ($app.HasExited) { throw 'Store binary exited before smoke test' }
    $synthetic = 'ClipCat Store smoke: local clipboard sample 2026-11-03'
    Set-Clipboard -Value $synthetic
    $history = Join-Path $testData 'ClipCat/clips.json'
    # The app intentionally batches persistence on a 30-second autosave cadence.
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ((Test-Path $history) -and ((Get-Content $history -Raw).Contains($synthetic))) { break }
        Start-Sleep -Milliseconds 500
    }
    if (!(Test-Path $history) -or !((Get-Content $history -Raw).Contains($synthetic))) { throw 'Synthetic clipboard item was not persisted' }
    $hwnd = [ClipCatSmoke]::FindWindow('ClipCatWindow', 'ClipCat')
    if ($hwnd -eq [IntPtr]::Zero) { throw 'Native cat window not found' }
    # Open the real embedded panel through its middle-button event.
    [ClipCatSmoke]::PostMessage($hwnd, 0x0208, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Seconds 1
    $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bounds.Size)
        $bitmap.Save((Join-Path $out 'windows-native-panel.png'), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
    @{ scope = 'staged Store executable, native Windows window, synthetic clipboard persistence'; msix_installed = $false; passed = $true } | ConvertTo-Json | Set-Content (Join-Path $out 'smoke.json')
} finally {
    if ($app -and !$app.HasExited) {
        $hwnd = [ClipCatSmoke]::FindWindow('ClipCatWindow', 'ClipCat')
        [ClipCatSmoke]::PostMessage($hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
        if (!$app.WaitForExit(5000)) { $app.Kill() }
    }
    $env:APPDATA = $previousAppData
}
