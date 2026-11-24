@echo off
setlocal EnableExtensions
cd /d "%~dp0"

rem Usage: release.bat 0.8.1  (or double-click it and type the version)
set "PROMPTED="
set "V=%~1"
if "%V%"=="" set "PROMPTED=1"
if "%V%"=="" set /p "V=Version (e.g. 0.8.1): "
if /i "%V:~0,1%"=="v" set "V=%V:~1%"
if "%V%"=="" goto :badver
set "ST_VERSION=%V%"
powershell -NoProfile -Command "if ($env:ST_VERSION -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$'){exit 1}"
if errorlevel 1 goto :badver

set "REPO=Parlaxz/Stream-Tube-Desktop"

for /f "delims=" %%i in ('git rev-parse --abbrev-ref HEAD') do set "BRANCH=%%i"
if /i not "%BRANCH%"=="custom" (
  echo Run this from the custom branch.
  goto :fail
)
set "DIRTY="
for /f "delims=" %%i in ('git status --porcelain') do set "DIRTY=1"
if defined DIRTY (
  echo Working tree is not clean. Commit or stash your changes first.
  goto :fail
)
git rev-parse -q --verify "refs/tags/v%V%" >nul
if not errorlevel 1 (
  echo Tag v%V% already exists.
  goto :fail
)

set "CARGO=cargo"
where cargo >nul 2>nul || set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
if not exist "%CARGO%" if /i not "%CARGO%"=="cargo" (
  echo cargo not found.
  goto :fail
)
if not exist "third_party\mpv\libmpv-2.dll" (
  echo third_party\mpv\libmpv-2.dll is missing.
  goto :fail
)
set "ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" set "ISCC=%ProgramFiles%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" (
  echo Inno Setup 6 not found.
  goto :fail
)
where gh >nul 2>nul || (echo gh not found. & goto :fail)
gh auth status >nul 2>&1 || (echo gh is not signed in. & goto :fail)

echo Bumping Cargo.toml to %V%
powershell -NoProfile -Command "$p='Cargo.toml'; $q=[char]34; $t=[IO.File]::ReadAllText($p); $n=[regex]::Replace($t,'(?m)^version = .*$',('version = '+$q+$env:ST_VERSION+$q)); [IO.File]::WriteAllText($p,$n)"
if errorlevel 1 (
  echo Could not update Cargo.toml.
  goto :fail
)

echo Building release...
"%CARGO%" build --release || (git checkout -- Cargo.toml Cargo.lock & goto :fail)

echo Building installer...
"%ISCC%" /Q "/DAppVersion=%V%" "installer\streamtube.iss" || (git checkout -- Cargo.toml Cargo.lock & goto :fail)

git add Cargo.toml Cargo.lock
git commit -m "Version %V%" || goto :fail
git push origin custom || goto :fail
git tag "v%V%" || goto :fail
git push origin "v%V%" || goto :fail

set "PRE="
set "LATEST=--latest"
echo %V%| findstr /c:"-" >nul
if not errorlevel 1 set "PRE=--prerelease"
if not errorlevel 1 set "LATEST="

set "NOTES=--generate-notes"
powershell -NoProfile -Command "$v=$env:ST_VERSION; $t=[IO.File]::ReadAllText('CHANGELOG.md'); $m=[regex]::Match($t,'(?ms)^##\s+'+[regex]::Escape($v)+'\s*\r?\n(.*?)(?=^##\s|\z)'); $b=$m.Groups[1].Value.Trim(); if(-not $m.Success -or $b.Length -eq 0){exit 1}; [IO.File]::WriteAllText('dist\release-notes.md',$b,(New-Object System.Text.UTF8Encoding($false)))"
if not errorlevel 1 set "NOTES=--notes-file dist\release-notes.md"

gh release create "v%V%" "dist\StreamTube-%V%-setup.exe" --repo %REPO% --verify-tag --title "Stream Tube %V%" %NOTES% %LATEST% %PRE% || goto :fail

echo Done: https://github.com/%REPO%/releases/tag/v%V%
goto :end

:badver
echo Invalid version. Use a form like 0.8.1 or 0.8.1-alpha.
goto :fail

:fail
if defined PROMPTED pause
exit /b 1

:end
if defined PROMPTED pause
exit /b 0
