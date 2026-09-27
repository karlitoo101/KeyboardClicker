$env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH
$log = "$env:TEMP\cargotest.txt"
"TEST START $(Get-Date -Format o)" | Out-File $log
Set-Location "c:\Users\karlm\Downloads\KeyboardClicker"
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test 2>&1 | Out-File $log -Append
"CARGO EXIT: $LASTEXITCODE" | Out-File $log -Append
"TEST DONE $(Get-Date -Format o)" | Out-File $log -Append
