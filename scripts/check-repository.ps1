[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$failures = [System.Collections.Generic.List[string]]::new()

$textExtensions = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)
@(
    '.css', '.html', '.js', '.json', '.jsonc', '.md', '.ps1', '.rs', '.sh',
    '.toml', '.ts', '.tsx', '.txt', '.yaml', '.yml'
) | ForEach-Object { [void]$textExtensions.Add($_) }

$textFileNames = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)
@(
    '.editorconfig', '.gitattributes', '.gitignore', 'Cargo.lock', 'LICENSE',
    'NOTICE'
) | ForEach-Object {
    [void]$textFileNames.Add($_)
}

$ignoredDirectoryNames = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)
@('.git', 'node_modules', 'target') | ForEach-Object {
    [void]$ignoredDirectoryNames.Add($_)
}

function Get-RelativeRepositoryPath {
    param([Parameter(Mandatory)] [string] $FullPath)

    $rootPrefix = $repositoryRoot
    $separator = [System.IO.Path]::DirectorySeparatorChar
    $altSeparator = [System.IO.Path]::AltDirectorySeparatorChar
    if (-not $rootPrefix.EndsWith($separator) -and
        -not $rootPrefix.EndsWith($altSeparator)) {
        $rootPrefix += $separator
    }

    if ($FullPath.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $FullPath.Substring($rootPrefix.Length)
    }

    return $FullPath
}

function Get-RepositoryTextFiles {
    $pendingDirectories = [System.Collections.Generic.Stack[string]]::new()
    $pendingDirectories.Push($repositoryRoot)

    while ($pendingDirectories.Count -gt 0) {
        $directory = $pendingDirectories.Pop()
        foreach ($entry in Get-ChildItem -LiteralPath $directory -Force) {
            if ($entry.PSIsContainer) {
                if (-not $ignoredDirectoryNames.Contains($entry.Name)) {
                    $pendingDirectories.Push($entry.FullName)
                }
                continue
            }

            if ($textExtensions.Contains($entry.Extension) -or
                $textFileNames.Contains($entry.Name)) {
                Write-Output $entry
            }
        }
    }
}

$files = @(Get-RepositoryTextFiles)

foreach ($file in $files) {
    $relativePath = Get-RelativeRepositoryPath -FullPath $file.FullName
    $content = [System.IO.File]::ReadAllText($file.FullName)

    if ($content -match '[\u2013\u2014]') {
        $failures.Add("$relativePath contains a prohibited dash character.")
    }

    $emojiPattern = '[\u2600-\u27BF\u2B00-\u2BFF\u20E3\uFE0F]|' +
        '[\uD83C-\uD83E][\uDC00-\uDFFF]'
    if ($content -match $emojiPattern) {
        $failures.Add("$relativePath contains a prohibited emoji code point.")
    }

    $attributionPatterns = @(
        ('(?i)(generated|written|created|authored|implemented)\s+by\s+' +
            '(codex|claude|chatgpt|copilot)'),
        '(?i)co-authored-by\s*:.*(codex|claude|chatgpt|copilot)'
    )

    foreach ($pattern in $attributionPatterns) {
        if ($content -match $pattern) {
            $failures.Add("$relativePath contains prohibited authorship attribution.")
            break
        }
    }

    if ($file.Extension -ieq '.rs' -and $relativePath -match '(^|[\\/])src[\\/]') {
        $effectiveLines = @(
            [System.IO.File]::ReadAllLines($file.FullName) | Where-Object {
                $trimmed = $_.Trim()
                $trimmed.Length -gt 0 -and
                    -not $trimmed.StartsWith('//') -and
                    -not $trimmed.StartsWith('/*') -and
                    -not $trimmed.StartsWith('*') -and
                    -not $trimmed.StartsWith('*/')
            }
        ).Count

        if ($effectiveLines -gt 500) {
            $failures.Add(
                "$relativePath has $effectiveLines effective lines; the limit is 500."
            )
        }
    }

    if (($file.Extension -ieq '.ts' -or $file.Extension -ieq '.tsx') -and
        $relativePath -match '(^|[\\/])src[\\/]' -and
        $relativePath -notmatch '(?i)(^|[\\/])(__tests__|fixtures|generated)[\\/]' -and
        $relativePath -notmatch '(?i)\.(test|spec)\.(ts|tsx)$') {
        $effectiveLines = @(
            [System.IO.File]::ReadAllLines($file.FullName) | Where-Object {
                $trimmed = $_.Trim()
                $trimmed.Length -gt 0 -and
                    -not $trimmed.StartsWith('//') -and
                    -not $trimmed.StartsWith('/*') -and
                    -not $trimmed.StartsWith('*') -and
                    -not $trimmed.StartsWith('*/')
            }
        ).Count

        $lineLimit = if ($file.Extension -ieq '.tsx') { 200 } else { 350 }
        if ($effectiveLines -gt $lineLimit) {
            $failures.Add(
                "$relativePath has $effectiveLines effective lines; the " +
                "TypeScript limit is $lineLimit."
            )
        }
    }
}

$markdownFiles = $files | Where-Object { $_.Extension -ieq '.md' }
foreach ($file in $markdownFiles) {
    $content = [System.IO.File]::ReadAllText($file.FullName)
    $linkMatches = [regex]::Matches($content, '\[[^\]]+\]\((?<target>[^)]+)\)')

    foreach ($linkMatch in $linkMatches) {
        $target = $linkMatch.Groups['target'].Value.Trim()
        if ($target.StartsWith('<') -and $target.EndsWith('>')) {
            $target = $target.Substring(1, $target.Length - 2)
        }

        $targetWithoutFragment = ($target -split '#', 2)[0]
        if ([string]::IsNullOrWhiteSpace($targetWithoutFragment) -or
            $targetWithoutFragment -match '^[a-zA-Z][a-zA-Z0-9+.-]*:') {
            continue
        }

        $decodedTarget = [System.Uri]::UnescapeDataString($targetWithoutFragment)
        $resolvedTarget = Join-Path -Path $file.DirectoryName -ChildPath $decodedTarget
        if (-not (Test-Path -LiteralPath $resolvedTarget)) {
            $relativePath = Get-RelativeRepositoryPath -FullPath $file.FullName
            $failures.Add("$relativePath links to missing local target $target.")
        }
    }
}

$cargoManifest = Join-Path -Path $repositoryRoot -ChildPath 'Cargo.toml'
if (Test-Path -LiteralPath $cargoManifest) {
    foreach ($requiredRustFile in @('Cargo.lock', 'deny.toml')) {
        $requiredPath = Join-Path -Path $repositoryRoot -ChildPath $requiredRustFile
        if (-not (Test-Path -LiteralPath $requiredPath)) {
            $failures.Add(
                "Rust workspace requires repository-root $requiredRustFile."
            )
        }
    }
}

if ($failures.Count -gt 0) {
    $failures | Sort-Object -Unique | ForEach-Object {
        Write-Error $_ -ErrorAction Continue
    }
    exit 1
}

Write-Output "Repository policy checks passed for $($files.Count) text files."
