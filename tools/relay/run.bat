@echo off
rem Runs the relay in a restart loop on the NucBox. Edit PORT and TOKEN below.
rem TOKEN must match the value friends enter in Stream Tube. Keep it long and
rem secret; anyone with it can use the relay.
set "PORT=8930"
set "TOKEN=change-me-to-a-long-random-string"
cd /d "%~dp0"
:loop
echo [%date% %time%] starting relay on port %PORT%
relay.exe --port %PORT% --token "%TOKEN%"
echo relay exited; restarting in 5 seconds
timeout /t 5 /nobreak >nul
goto loop
