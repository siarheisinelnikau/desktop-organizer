param([string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $repository
try {
    if (!$OutputDirectory) { $OutputDirectory = Join-Path $repository 'docs/screenshots' }
    $OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
    $fixture = Join-Path $repository '.local/SampleDesktop'
    New-Item -ItemType Directory -Path $fixture -Force | Out-Null
    $files = @{
        'Project brief.txt' = 'Synthetic project brief for DesktopOrganizer visual checks.'
        'Meeting notes.md' = '# Meeting notes - Synthetic fixture only.'
        'Ideas.txt' = 'A few synthetic ideas for a calmer desktop.'
        'Budget.csv' = "Category,Amount`nDesign,120`nReview,80"
        'Tasks.json' = '{"tasks":["Review layout","Capture screenshots"]}'
        'Landscape.svg' = '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"><rect width="320" height="180" fill="#345b70"/></svg>'
    }
    foreach ($name in $files.Keys) { [IO.File]::WriteAllText((Join-Path $fixture $name), $files[$name]) }
    & cargo build --locked --offline --release --jobs 2
    if ($LASTEXITCODE -ne 0) { throw 'Capture build failed.' }
    $executable = Join-Path $repository 'target/release/DesktopOrganizer.exe'
    foreach ($name in @('library', 'organize', 'settings', 'group', 'settings-ai')) {
        $screen = if ($name -eq 'settings-ai') { 'settings' } else { $name }
        $output = Join-Path $OutputDirectory "$name.png"
        $started = [DateTime]::UtcNow
        $arguments = @('--capture', ('"' + $output + '"'), '--screen', $screen, '--fixture-root', ('"' + $fixture + '"'))
        if ($name -eq 'settings-ai') { $arguments += @('--scenario', 'http-server') }
        $process = Start-Process -FilePath $executable -WorkingDirectory $repository -ArgumentList $arguments -WindowStyle Hidden -PassThru
        if (!$process.WaitForExit(15000)) { Stop-Process -Id $process.Id; throw "Capture timed out: $screen" }
        if ($process.ExitCode -ne 0) { throw "Capture failed: $screen (exit $($process.ExitCode))" }
        $image = Get-Item -LiteralPath $output
        if ($image.LastWriteTimeUtc -lt $started) { throw "Capture file is stale: $screen" }
        $bytes = [IO.File]::ReadAllBytes($output)
        if ($bytes.Length -lt 24 -or $bytes[0] -ne 137 -or $bytes[1] -ne 80 -or $bytes[2] -ne 78 -or $bytes[3] -ne 71) { throw "Invalid PNG: $screen" }
        Write-Output "Saved actual $name client area; capture PID $($process.Id) exited successfully."
    }
} finally { Pop-Location }
