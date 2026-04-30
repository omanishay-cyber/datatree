param([string]$Out = "C:\x2-corpus", [int]$N = 50)
$ErrorActionPreference = "Continue"
if (-not (Test-Path $Out)) {
    New-Item -ItemType Directory -Path $Out -Force | Out-Null
}
1..$N | ForEach-Object {
    $i = $_
    $body = "pub fn f$i() -> u32 { $i }`n"
    Set-Content -Path (Join-Path $Out "f$i.rs") -Value $body -Encoding UTF8
}
$count = (Get-ChildItem $Out -File).Count
Write-Host "seeded=$count at=$Out"
