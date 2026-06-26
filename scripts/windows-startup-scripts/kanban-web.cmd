@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "PS_SCRIPT=%SCRIPT_DIR%kanban-web.ps1"

if not exist "%PS_SCRIPT%" (
    echo kanban-web.ps1 was not found next to this CMD file.
    echo Expected: "%PS_SCRIPT%"
    exit /b 1
)

rem No arguments: preserve the double-click behavior from the original wrapper.
if "%~1"=="" (
    start "Kanban Web ip-2.0" powershell.exe -NoExit -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%" start -Foreground -NoOpen
    exit /b 0
)

rem Arguments provided: run the requested command and return its exit code.
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%" %*
exit /b %ERRORLEVEL%
