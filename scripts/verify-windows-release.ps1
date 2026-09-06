param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("x86_64", "arm64")]
    [string]$Architecture,

    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)]
    [string[]]$Binary
)

$ErrorActionPreference = "Stop"
$dumpbin = (Get-Command dumpbin.exe -ErrorAction Stop).Source
$expectedMachine = if ($Architecture -eq "x86_64") {
    '8664\s+machine\s+\(x64\)'
} else {
    'AA64\s+machine\s+\(ARM64\)'
}
$redistributable = '(?im)^\s*(vcruntime|msvcp|concrt|ucrtbased?|api-ms-win-crt)[^\s]*\.dll\s*$'
$dependency = '(?im)^\s*([A-Za-z0-9._-]+\.dll)\s*$'

foreach ($path in $Binary) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Windows executable is missing: $path"
    }

    $headers = & $dumpbin /HEADERS $path | Out-String
    if ($LASTEXITCODE -ne 0 -or $headers -notmatch $expectedMachine) {
        throw "$path is not a Windows $Architecture executable"
    }

    $dependents = & $dumpbin /DEPENDENTS $path | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "dumpbin failed to read dependencies from $path"
    }
    if ($dependents -match $redistributable) {
        throw "$path dynamically links a redistributable CRT library: $($Matches[0].Trim())"
    }

    $names = @(
        [regex]::Matches($dependents, $dependency) |
            ForEach-Object { $_.Groups[1].Value } |
            Sort-Object -Unique
    )
    if ($names.Count -eq 0) {
        throw "dumpbin reported no Windows system DLL dependencies for $path"
    }
    foreach ($name in $names) {
        if ($name -match '^(api|ext)-ms-win-[A-Za-z0-9-]+\.dll$') {
            continue
        }
        if (-not (Test-Path -LiteralPath (Join-Path $env:SystemRoot "System32/$name") -PathType Leaf)) {
            throw "$path has a non-system dynamic dependency: $name"
        }
    }
}

Write-Host "verified Windows $Architecture executables with static CRT and only system DLL imports: $Binary"
