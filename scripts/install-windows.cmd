@echo off
setlocal
if /I "%~1"=="quiet" set "MD_PREVIEWER_INSTALL_QUIET=1"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-windows.ps1"
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
  echo.
  echo MD Previewer installation failed with exit code %EXIT_CODE%.
  if not "%MD_PREVIEWER_INSTALL_QUIET%"=="1" pause
)
exit /b %EXIT_CODE%
