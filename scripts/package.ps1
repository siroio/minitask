#requires -Version 5.1
param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$target = 'x86_64-pc-windows-msvc'
$previousFlags = $env:CARGO_ENCODED_RUSTFLAGS
$stage = Join-Path $repo ('.cache/package-' + [guid]::NewGuid().ToString('N'))
$utf8 = New-Object System.Text.UTF8Encoding($false)
Push-Location $repo
try {
    if ($env:OS -ne 'Windows_NT') { throw 'This script builds the Windows x64 package.' }
    $cargoArgs = @('--locked')
    if ($Offline) { $cargoArgs += '--offline' }
    $raw = & cargo metadata @cargoArgs --format-version 1 --filter-platform $target
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $metadata = ($raw -join "`n") | ConvertFrom-Json
    $package = $metadata.packages | Where-Object id -eq $metadata.resolve.root
    $sysroot = & rustc --print sysroot
    if ($LASTEXITCODE -ne 0) { throw 'rustc failed' }
    # Static CRT avoids requiring a separate Visual C++ runtime installation.
    $env:CARGO_ENCODED_RUSTFLAGS = (@('-C', 'target-feature=+crt-static',
        "--remap-path-prefix=$repo=minitask", "--remap-path-prefix=$sysroot=rust") -join [char]31)
    & cargo build @cargoArgs --release --target $target --target-dir target/package
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    $name = "minitask-$($package.version)-windows-x64"
    $bundle = Join-Path $stage $name
    New-Item -ItemType Directory -Path $bundle -Force | Out-Null
    $files = @{
        "target/package/$target/release/minitask.exe" = 'minitask.exe'
        'DISTRIBUTION.md' = 'README.md'
        'LICENSE' = 'LICENSE'
        'locales/ja.json' = 'locales/ja.json'
        'emacs/minitask.el' = 'emacs/minitask.el'
        'emacs/locales/ja.json' = 'emacs/locales/ja.json'
        'docs/emacs.md' = 'docs/emacs.md'
        'skills/using-minitask/SKILL.md' = 'skills/using-minitask/SKILL.md'
        'yazi-keymap.toml' = 'yazi-keymap.toml'
    }
    foreach ($source in $files.Keys) {
        $destination = Join-Path $bundle $files[$source]
        New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $repo $source) -Destination $destination
    }
    $notices = @('Third-party components (including build dependencies).',
                 'License expressions below are upstream declarations; full texts are in licenses/.', '')
    foreach ($dependency in ($metadata.packages | Where-Object { $_.source -and $_.id -in $metadata.resolve.nodes.id } | Sort-Object name,version)) {
        $directory = Split-Path $dependency.manifest_path
        $label = "$($dependency.name)-$($dependency.version)"
        $licenseFiles = @(Get-ChildItem -LiteralPath $directory -File -Recurse |
            Where-Object Name -Match '^(LICENSE|LICENCE|COPYING|COPYRIGHT|NOTICE)([._-]|$)')
        if ($dependency.license_file) { $licenseFiles += Get-Item -LiteralPath (Join-Path $directory $dependency.license_file) }
        if (!$licenseFiles) { throw "No license text found for $label" }
        $notices += "$label : $($dependency.license)"
        foreach ($file in ($licenseFiles | Sort-Object FullName -Unique)) {
            $relative = $file.FullName.Substring($directory.Length + 1)
            $destination = Join-Path $bundle "licenses/$label/$relative"
            New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $destination
        }
    }
    $rustDocs = Join-Path $sysroot 'share/doc/rust'
    $rustDestination = Join-Path $bundle 'licenses/rust'
    New-Item -ItemType Directory -Path $rustDestination -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $rustDocs 'COPYRIGHT-library.html') -Destination $rustDestination
    Copy-Item -LiteralPath (Join-Path $rustDocs 'licenses') -Destination $rustDestination -Recurse
    $notices += "`nRust standard library: $(rustc --version). See licenses/rust/."
    [IO.File]::WriteAllText((Join-Path $bundle 'THIRD-PARTY-NOTICES.txt'), ($notices -join "`n"), $utf8)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = Join-Path $stage "$name.zip"
    [IO.Compression.ZipFile]::CreateFromDirectory($bundle, $zip, [IO.Compression.CompressionLevel]::Optimal, $true)
    $checksum = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$zip.sha256", "$checksum  $name.zip`n", $utf8)
    $publish = Join-Path $repo 'publish'
    New-Item -ItemType Directory -Path $publish -Force | Out-Null
    Move-Item -LiteralPath $zip -Destination (Join-Path $publish "$name.zip") -Force
    Move-Item -LiteralPath "$zip.sha256" -Destination (Join-Path $publish "$name.zip.sha256") -Force
    Write-Output (Join-Path $publish "$name.zip")
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS = $previousFlags
    Pop-Location
    $stage = [IO.Path]::GetFullPath($stage)
    $cachePrefix = [IO.Path]::GetFullPath((Join-Path $repo '.cache')) + [IO.Path]::DirectorySeparatorChar
    if (!$stage.StartsWith($cachePrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe staging path' }
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
}
