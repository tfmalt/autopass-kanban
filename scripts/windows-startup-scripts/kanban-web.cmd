@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "PS_SCRIPT=%SCRIPT_DIR%kanban-web.ps1"

if not exist "%PS_SCRIPT%" (
    echo kanban-web.ps1 was not found next to this CMD file.
    echo Expected: "%PS_SCRIPT%"
    exit /b 1
)

rem No arguments: complete an idempotent background restart before returning.
if "%~1"=="" (
    powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%" restart -NoOpen
    exit /b %ERRORLEVEL%
)

rem Arguments provided: run the requested command and return its exit code.
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%" %*
exit /b %ERRORLEVEL%
