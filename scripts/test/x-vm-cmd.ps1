param(
    [Parameter(Mandatory=$true)] [string]$Cmd
)
$ErrorActionPreference = "Continue"
& "C:\Program Files\PuTTY\plink.exe" -ssh -batch `
    -hostkey "SHA256:ugWmFV7VQt1gHIFsa5CisYx7a8NSUhEGr1J53IZm+fo" `
    -pw "y!mPt3NwW%vIFsM@2QFBKynusqwTekxe" `
    Administrator@13.59.120.55 `
    "powershell -NoProfile -Command `"$Cmd`""
