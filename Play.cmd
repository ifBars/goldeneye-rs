@echo off
cd /d "%~dp0"
if not exist "target\release\goldeneye.exe" (
  echo Build the native game first with: cargo xtask check
  pause
  exit /b 1
)
"target\release\goldeneye.exe" play %*
if errorlevel 1 pause
