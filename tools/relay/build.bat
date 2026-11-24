@echo off
rem Builds relay.exe next to this file. Run this on the machine that has Rust;
rem then copy relay.exe + run.bat to the NucBox.
cd /d "%~dp0"
set "RUSTC=rustc"
where rustc >nul 2>nul || set "RUSTC=%USERPROFILE%\.cargo\bin\rustc.exe"
"%RUSTC%" --edition 2021 -O relay.rs -o relay.exe
if errorlevel 1 (
  echo Build failed.
  pause
  exit /b 1
)
echo Built %~dp0relay.exe
pause
