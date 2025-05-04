$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $repository
try {
    & cargo fmt --all --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting failed.' }
    & cargo clippy --locked --offline --all-targets --jobs 2 -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy failed.' }
    # Runs Rust test harnesses only; never starts the desktop application.
    & cargo test --locked --offline --lib --jobs 2
    if ($LASTEXITCODE -ne 0) { throw 'Fixture tests failed.' }
    & cargo build --locked --offline --release --jobs 2
    if ($LASTEXITCODE -ne 0) { throw 'Release compilation failed.' }
} finally { Pop-Location }
