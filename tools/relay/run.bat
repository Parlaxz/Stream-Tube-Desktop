@echo off
rem Runs the relay in a restart loop. The token is asked once and saved to
rem token.txt next to this file (delete that file to set a new token).
rem Friends enter the same token in Stream Tube. Long and random is best.
set "PORT=8930"
cd /d "%~dp0"
if exist token.txt goto have
echo No token yet. Choose the token your friends will enter in Stream Tube.
set /p TOKEN=Token: 
if "%TOKEN%"=="" (
  echo No token entered; aborting.
  pause
  exit /b 1
)
>token.txt echo %TOKEN%
:have
set /p TOKEN=<token.txt
:loop
echo [%date% %time%] starting relay on port %PORT% (token: %TOKEN%)
relay.exe --port %PORT% --token "%TOKEN%"
echo relay exited; restarting in 5 seconds
timeout /t 5 /nobreak >nul
goto loop
