@echo off
setlocal
pushd "%~dp0"

rem Never replace a running app or create alternative executable names.
powershell.exe -NoProfile -Command "$wrenchProcesses = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match '^eve[-_]wrench(?:[-_.]|$)' }); if ($wrenchProcesses.Count -gt 0) { Write-Warning ('EVE Wrench is running: ' + (($wrenchProcesses | ForEach-Object { $_.ProcessName + ' (PID ' + $_.Id + ')' }) -join ', ')); Write-Output 'Build skipped. Close EVE Wrench, then run build.cmd again.'; exit 2 }"
if errorlevel 1 (
    popd
    exit /b 2
)

call cargo build --locked --release -p eve-wrench %*
set "wrenchBuildExit=%ERRORLEVEL%"
if "%wrenchBuildExit%"=="0" echo Built: %CD%\target\release\eve-wrench.exe
popd
exit /b %wrenchBuildExit%
