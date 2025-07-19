$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $repository
try {
    $metadataText = & cargo metadata --locked --offline --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve locked dependencies offline. Run cargo fetch --locked when dependency downloads are permitted.' }
    $metadata = ($metadataText -join "`n") | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.name -eq 'desktop-organizer' }
    $version = $package.version
    [xml]$manifest = Get-Content -LiteralPath 'assets/app.manifest' -Raw
    if ($manifest.assembly.assemblyIdentity.version -ne "$version.0") { throw 'Cargo and application manifest versions differ.' }
    & cargo build --locked --offline --release --jobs 2
    if ($LASTEXITCODE -ne 0) { throw 'Release compilation failed.' }
    $binary = Join-Path $repository 'target/release/DesktopOrganizer.exe'
    $reader = [System.IO.BinaryReader]::new([System.IO.File]::OpenRead($binary))
    try {
        $reader.BaseStream.Position = 0x3c
        $offset = $reader.ReadInt32()
        $reader.BaseStream.Position = $offset + 4
        if ($reader.ReadUInt16() -ne 0x8664) { throw 'The initial release must be a Windows x64 executable.' }
    } finally { $reader.Dispose() }
    $output = Join-Path $repository 'dist'
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    $name = "DesktopOrganizer-v$version.exe"
    Copy-Item -LiteralPath $binary -Destination (Join-Path $output $name) -Force
    Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $output 'LICENSE') -Force
    $hash = (Get-FileHash -LiteralPath (Join-Path $output $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    [System.IO.File]::WriteAllText((Join-Path $output 'SHA256SUMS'), "$hash  $name`n", [System.Text.UTF8Encoding]::new($false))
    Copy-Item -LiteralPath '../rust-auto-updator/LICENSE' -Destination (Join-Path $output 'LICENSE-rust-auto-updator.txt') -Force
    $notices = [System.Text.StringBuilder]::new()
    [void]$notices.AppendLine('Third-party notices for the complete Cargo.lock dependency graph (including build/test and other-platform packages).')
    foreach ($dependency in ($metadata.packages | Sort-Object name, version)) {
        if ($dependency.name -eq 'desktop-organizer') { continue }
        [void]$notices.AppendLine("`n===== $($dependency.name) $($dependency.version) | $($dependency.license) =====")
        $directory = Split-Path -Parent $dependency.manifest_path
        $files = Get-ChildItem -LiteralPath $directory -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)([._-].*)?$' }
        foreach ($file in $files) {
            [void]$notices.AppendLine("`n--- $($file.Name) ---")
            [void]$notices.AppendLine([System.IO.File]::ReadAllText($file.FullName))
        }
    }
    [System.IO.File]::WriteAllText((Join-Path $output 'THIRD-PARTY-LICENSES.txt'), $notices.ToString(), [System.Text.UTF8Encoding]::new($false))
    Write-Output "Prepared $name and SHA256SUMS in $output. No application was launched and nothing was published."
} finally { Pop-Location }
