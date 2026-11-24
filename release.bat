@echo off
setlocal EnableExtensions
cd /d "%~dp0"

rem Usage: release.bat 0.8.1  (or run it and type the version)
set "V=%~1"
if "%V%"=="" set /p "V=Version (e.g. 0.8.1): "
if /i "%V:~0,1%"=="v" set "V=%V:~1%"
if "%V%"=="" goto :badver
set "ST_VERSION=%V%"
powershell -NoProfile -Command "if ($env:ST_VERSION -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$'){exit 1}"
if errorlevel 1 goto :badver

for /f "delims=" %%i in ('git rev-parse --abbrev-ref HEAD') do set "BRANCH=%%i"
if /i not "%BRANCH%"=="custom" (
  echo Run this from the custom branch.
  exit /b 1
)
for /f "delims=" %%i in ('git status --porcelain --untracked-files=no') do (
  echo Working tree is not clean. Commit or stash your changes first.
  exit /b 1
)
git rev-parse -q --verify "refs/tags/v%V%" >nul
if not errorlevel 1 (
  echo Tag v%V% already exists.
  exit /b 1
)

set "CARGO=cargo"
where cargo >nul 2>nul || set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
if not exist "%CARGO%" if /i not "%CARGO%"=="cargo" (
  echo cargo not found.
  exit /b 1
)
if not exist "third_party\mpv\libmpv-2.dll" (
  echo third_party\mpv\libmpv-2.dll is missing.
  exit /b 1
)
set "ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" set "ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" (
  echo Inno Setup 6 not found.
  exit /b 1
)
where gh >nul 2>nul || (echo gh not found. & exit /b 1)
gh auth status >nul 2>&1 || (echo gh is not signed in. & exit /b 1)

echo Bumping Cargo.toml to %V%
powershell -NoProfile -Command "$p='Cargo.toml'; $q=[char]34; $t=[IO.File]::ReadAllText($p); $n=[regex]::Replace($t,'(?m)^version = .*$',('version = '+$q+$env:ST_VERSION+$q)); [IO.File]::WriteAllText($p,$n)"
if errorlevel 1 (
  echo Could not update Cargo.toml.
  exit /b 1
)

echo Building release...
"%CARGO%" build --release || (git checkout -- Cargo.toml Cargo.lock & exit /b 1)

echo Building installer...
"%ISCC%" /Q "/DAppVersion=%V%" "installer\streamtube.iss" || (git checkout -- Cargo.toml Cargo.lock & exit /b 1)

git add Cargo.toml Cargo.lock
git commit -m "Version %V%" || exit /b 1
git push origin custom || exit /b 1
git tag "v%V%" || exit /b 1
git push origin "v%V%" || exit /b 1

set "PRE="
set "LATEST=--latest"
echo %V%| findstr /c:"-" >nul
if not errorlevel 1 set "PRE=--prerelease"
if not errorlevel 1 set "LATEST="
gh release create "v%V%" "dist\StreamTube-%V%-setup.exe" --verify-tag --title "Stream Tube %V%" --generate-notes %LATEST% %PRE% || exit /b 1

echo Done: https://github.com/Parlaxz/Stream-Tube-Desktop/releases/tag/v%V%
if "%~1"=="" pause
exit /b 0

:badver
echo Invalid version. Use a form like 0.8.1 or 0.8.1-alpha.
if "%~1"=="" pause
exit /b 1
