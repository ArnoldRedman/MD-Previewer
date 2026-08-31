@echo off
setlocal
cd /d "%~dp0"

set "NO_PAUSE="
if /I "%~1"=="--no-pause" set "NO_PAUSE=1"

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-windows.ps1"
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
  echo.
  echo Windows build failed with exit code %EXIT_CODE%.
) else (
  echo.
  echo Windows build completed successfully.
)

if not defined NO_PAUSE pause
exit /b %EXIT_CODE%
