param([string]$OutputDirectory, [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $repository
try {
    if (!$OutputDirectory) { $OutputDirectory = Join-Path $repository 'docs/screenshots' }
    $OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
    $fixture = Join-Path $repository '.local/PurposeDesktop'
    New-Item -ItemType Directory -Path $OutputDirectory,$fixture -Force | Out-Null
    foreach ($name in @('Mozilla Firefox.lnk','Google Chrome.lnk','Visual Studio Code.lnk','PyCharm.lnk','Blender 4.5.lnk','Fusion 360.lnk','Steam.lnk','Telegram.lnk','Discord.lnk','VLC.lnk','DaVinci Resolve.lnk','Audacity.lnk','7-Zip File Manager.lnk','Unknown launcher.lnk','Project brief.txt','Budget.csv')) {
        [IO.File]::WriteAllText((Join-Path $fixture $name), 'Synthetic fixture only; never execute this reference.')
    }
    if (!$SkipBuild) {
        & cargo build --locked --offline --release --jobs 2
        if ($LASTEXITCODE -ne 0) { throw 'Capture build failed.' }
    }
    $captures = @(
        @{Name='organize-mixed-ru';Screen='organize';Language='ru';Theme='dark'},
        @{Name='conversation-ru';Screen='conversation';Language='ru';Theme='dark'},
        @{Name='arrangement-ru';Screen='arrangement';Language='ru';Theme='dark'},
        @{Name='organize';Screen='organize';Language='en';Theme='light'},
        @{Name='library';Screen='library';Language='en';Theme='light'},
        @{Name='moves';Screen='moves';Language='en';Theme='light'},
        @{Name='settings';Screen='settings';Language='en';Theme='light'}
    )
    $executable = Join-Path $repository 'target/release/DesktopOrganizer.exe'
    foreach ($capture in $captures) {
        $output = Join-Path $OutputDirectory ($capture.Name + '.png')
        $started = [DateTime]::UtcNow
        $arguments = @('--capture',('"'+$output+'"'),'--screen',$capture.Screen,'--fixture-root',('"'+$fixture+'"'),'--scenario','mixed','--language',$capture.Language,'--theme',$capture.Theme)
        $process = Start-Process -FilePath $executable -WorkingDirectory $repository -ArgumentList $arguments -WindowStyle Hidden -PassThru
        if (!$process.WaitForExit(15000)) { Stop-Process -Id $process.Id; throw "Capture timed out: $($capture.Name)" }
        if ($process.ExitCode -ne 0) { throw "Capture failed: $($capture.Name)" }
        if ((Get-Item -LiteralPath $output).LastWriteTimeUtc -lt $started) { throw "Stale capture: $output" }
        Write-Output "Saved $($capture.Name); PID $($process.Id) exited successfully."
    }
} finally { Pop-Location }
