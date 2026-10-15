@echo off
setlocal EnableExtensions DisableDelayedExpansion
rem issue121-test.bat: test build for issue #121 (AMD: the 3D scene only in a box in the top-left
rem corner). Starts the game with extra GPU diagnostics, keeps last_run.log, the frame captures and
rem the F12 screenshots in results\<date>\<run>\ and zips them to the Desktop. ASCII only, CRLF line
rem endings (.gitattributes). "issue121-test.bat A E" runs those without the menu, then zips.
rem Test hooks: T121_GAME replaces "God of War III.exe", T121_DESKTOP the zip's folder and
rem T121_PROBE the process name that means the game is already running (gow3-probe.exe),
rem T121_ZIP_LIMIT_MB the size above which a second zip without screenshots is made (24).
rem The saves folders (GOW3_USER_DIR, GOW3_DATA_DIR, GOW3_GPU_USER_DIR) are never changed here.
title God of War III - issue 121 test
cd /d "%~dp0"
set "PORT=%~dp0"
set "PORT=%PORT:~0,-1%"
set "PS=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
set "GAME=%T121_GAME%"
if not defined GAME set "GAME=%PORT%\God of War III.exe"
if not exist "%GAME%" (
    echo "God of War III.exe" is not next to this file.
    echo Extract the whole zip over your gow3-windows folder, then run issue121-test.bat from there.
    pause
    exit /b 1
)
set "PROBE=%T121_PROBE%"
if not defined PROBE set "PROBE=gow3-probe.exe"
rem find.exe by its full path: a Unix find earlier in PATH (Git, MSYS2) would skip this check.
tasklist /fi "imagename eq %PROBE%" 2>nul | "%SystemRoot%\System32\find.exe" /i "%PROBE%" >nul
if not errorlevel 1 (
    echo The game is already running. Close it, then start this file again.
    pause
    exit /b 1
)
for /f "delims=" %%T in ('%PS% -NoProfile -Command Get-Date -Format yyyyMMdd-HHmmss') do set "STAMP=%%T"
if not defined STAMP set "STAMP=session"
set "SESSION=%PORT%\results\%STAMP%"
if not "%~1"=="" goto unattended

:menu
cls
echo.
echo   God of War III - test build for issue #121
echo   ------------------------------------------
echo   Extract this build over your current gow3-windows folder: that keeps your
echo   saves and the shader cache. Keep the game at 1080p.
echo.
echo   Each run starts the game. Play at least 2 minutes of 3D in every run, even
echo   when the picture looks right: the main menu with Kratos, the opening, the
echo   first fight or the upgrade menu. Press F12 where the 3D shows, then close
echo   the game. Frame captures are made automatically; F11 adds one. Note for
echo   each run whether the 3D picture was only in the box in the top-left corner.
echo.
echo   1  Run A - diagnostics only. The most important one: do it first.
echo   2  Run B - viewport and scissor sent again before every draw
echo   3  Run C - depth/stencil state also given to the pipeline
echo   4  Run D - depth/stencil state sent again before every draw
echo   5  Run E - full memory barriers
echo   6  All runs, A to E, one after another
echo   Z  Zip the results to the Desktop and quit
echo   Q  Quit without zipping; the files stay in results\%STAMP%
echo.
if exist "%SESSION%\" (
    echo   Done so far:
    for /d %%D in ("%SESSION%\*") do echo     %%~nxD
    echo.
)
choice /c 123456ZQ /n /m "Choose 1-6, Z or Q: "
set "PICK=%ERRORLEVEL%"
if "%PICK%"=="7" goto zip
if "%PICK%"=="8" exit /b 0
if "%PICK%"=="1" call :one A
if "%PICK%"=="2" call :one B
if "%PICK%"=="3" call :one C
if "%PICK%"=="4" call :one D
if "%PICK%"=="5" call :one E
if "%PICK%"=="6" call :one ALL
echo.
echo   When you are done with the runs, choose Z to make the zip.
pause
goto menu

:unattended
for %%R in (%*) do call :one %%R
call :pack
exit /b %ERRORLEVEL%

:zip
call :pack
pause
exit /b 0

rem %1: a run's letter, or ALL for every run.
:one
if /i "%~1"=="A" call :run A-diagnostics
if /i "%~1"=="B" call :run B-viewport GOW3_STATE_PROBE viewport
if /i "%~1"=="C" call :run C-depthstruct GOW3_STATE_PROBE depthstruct
if /i "%~1"=="D" call :run D-depthreemit GOW3_STATE_PROBE depthreemit
if /i "%~1"=="E" call :run E-full-barriers GOW3_FULL_BARRIERS 1
if /i "%~1"=="ALL" for %%R in (A B C D E) do call :one %%R
for %%R in (A B C D E ALL) do if /i "%~1"=="%%R" exit /b 0
echo   Unknown run "%~1": use A, B, C, D, E or ALL.
exit /b 1

rem %1: the run's folder name; %2 and %3: the variable the run sets and its value (none for run A).
:run
set /a N=1
:run_name
set "RUN=%SESSION%\%~1"
if %N% gtr 1 set "RUN=%RUN%-%N%"
if exist "%RUN%" (
    set /a N+=1
    goto run_name
)
mkdir "%RUN%\captures"
type nul > "%RUN%\started.txt"
echo.
echo   Run %~1: starting the game.
echo   Play at least 2 minutes of 3D, press F12 there, then close the game.
setlocal
rem Only run A logs at info level (tens of MB); the variations need only their GPU: lines.
set "GOW3_GPU_LOG="
if "%~2"=="" set "GOW3_GPU_LOG=info"
set "GOW3_FRAME_CAPTURE_KEY=1"
set "GOW3_CAPTURE_DIR=%RUN%\captures"
rem Each run sees only its own variation, even when an earlier test build left one set.
set "GOW3_STATE_PROBE="
set "GOW3_FULL_BARRIERS="
if not "%~2"=="" set "%~2=%~3"
set GOW3_ >"%RUN%\environment.txt" 2>nul
call "%GAME%" --play 1>"%RUN%\console.log" 2>&1
endlocal & set "CODE=%ERRORLEVEL%"
>"%RUN%\exit_code.txt" echo %CODE%
set "T121_RUN=%RUN%"
"%PS%" -NoProfile -ExecutionPolicy Bypass -Command ^
 "$run = $env:T121_RUN; $mark = (Get-Item -LiteralPath (Join-Path $run 'started.txt')).LastWriteTime;" ^
 "$data = if ($env:GOW3_DATA_DIR) { $env:GOW3_DATA_DIR } else { $env:PORT };" ^
 "$user = ''; $cfg = Join-Path $env:APPDATA 'gow3-launcher\settings.json';" ^
 "if (Test-Path -LiteralPath $cfg) { try { $user = (Get-Content -Raw -Encoding UTF8 -LiteralPath $cfg -ErrorAction Stop | ConvertFrom-Json).user_dir } catch {} };" ^
 "if (-not $user) { $user = Join-Path $data 'user' };" ^
 "Set-Content -LiteralPath (Join-Path $run 'saves_folder.txt') -Value $user -Encoding UTF8;" ^
 "$con = Join-Path $run 'console.log'; $quiet = (-not (Test-Path -LiteralPath $con)) -or ((Get-Item -LiteralPath $con).Length -eq 0);" ^
 "$log = Join-Path $user 'last_run.log';" ^
 "if (-not $quiet) { }" ^
 "elseif (-not (Test-Path -LiteralPath $log)) { Write-Host ('  last_run.log not found in ' + $user) }" ^
 "elseif ((Get-Item -LiteralPath $log).LastWriteTime -lt $mark) { Write-Host ('  ' + $log + ' is from an earlier start: not copied') }" ^
 "else { Copy-Item -LiteralPath $log -Destination $run };" ^
 "$shots = Join-Path $user 'screenshots'; $dest = Join-Path $run 'screenshots';" ^
 "if (Test-Path -LiteralPath $shots) { New-Item -ItemType Directory -Force -Path $dest | Out-Null; Get-ChildItem -LiteralPath $shots -Filter *.png | Where-Object { $_.LastWriteTime -ge $mark } | Copy-Item -Destination $dest };" ^
 "$ini = if ($env:GOW3_CONFIG) { $env:GOW3_CONFIG } else { Join-Path $data 'gow3.ini' };" ^
 "if (Test-Path -LiteralPath $ini) { Copy-Item -LiteralPath $ini -Destination $run };" ^
 "$src = Join-Path $run 'console.log'; if (-not (Test-Path -LiteralPath $src) -or (Get-Item -LiteralPath $src).Length -eq 0) { $src = Join-Path $run 'last_run.log';" ^
 " if (Test-Path -LiteralPath $src) { Write-Host '  The game output came from last_run.log.' }" ^
 " else { Write-Host '  The game did not start from here (did the launcher window open?). Choose the game folder in God of War III.exe, then run this again.' } };" ^
 "if (Test-Path -LiteralPath $src) { Select-String -LiteralPath $src -Pattern '^(GPU: |Frame capture: |Screenshot: |Settings: |STOP|Runtime: )|<Error>|<Critical>' | ForEach-Object { $_.Line } | Set-Content -LiteralPath (Join-Path $run 'summary.txt') -Encoding UTF8 }"
echo   Run %~1 finished (exit code %CODE%). Files: "%RUN%"
exit /b 0

:pack
set "T121_SESSION=%SESSION%"
set "T121_STAMP=%STAMP%"
"%PS%" -NoProfile -ExecutionPolicy Bypass -Command ^
 "$session = $env:T121_SESSION;" ^
 "if (-not (Test-Path -LiteralPath $session)) { Write-Host '  No runs yet: nothing to zip.'; exit 1 };" ^
 "Get-CimInstance Win32_VideoController | Format-List Name, DriverVersion, DriverDate, VideoModeDescription | Out-File -LiteralPath (Join-Path $session 'system.txt') -Encoding UTF8;" ^
 "[Environment]::OSVersion.VersionString | Out-File -LiteralPath (Join-Path $session 'system.txt') -Encoding UTF8 -Append;" ^
 "$desk = $env:T121_DESKTOP; if (-not $desk) { $desk = [Environment]::GetFolderPath('Desktop') };" ^
 "if (-not $desk -or -not (Test-Path -LiteralPath $desk)) { $desk = Join-Path $env:USERPROFILE 'Desktop' };" ^
 "if (-not (Test-Path -LiteralPath $desk)) { $desk = Split-Path -Parent $session };" ^
 "$zip = Join-Path $desk ('gow3-issue121-' + $env:T121_STAMP + '.zip');" ^
 "try { Compress-Archive -LiteralPath $session -DestinationPath $zip -Force -ErrorAction Stop }" ^
 "catch { Remove-Item -LiteralPath $zip -ErrorAction SilentlyContinue; Write-Host ('  Could not make the zip: ' + $_.Exception.Message); Write-Host ('  Zip this folder by hand: ' + $session); exit 1 };" ^
 "$limit = 24MB; if ($env:T121_ZIP_LIMIT_MB) { $limit = [int64]$env:T121_ZIP_LIMIT_MB * 1MB };" ^
 "if ((Get-Item -LiteralPath $zip).Length -le $limit) { Write-Host ''; Write-Host ('  Attach this file to issue #121: ' + $zip); exit 0 };" ^
 "$text = Join-Path $desk ('gow3-issue121-' + $env:T121_STAMP + '-text.zip');" ^
 "$stage = Join-Path ([IO.Path]::GetTempPath()) ('gow3-issue121-text\' + $env:T121_STAMP);" ^
 "Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue; New-Item -ItemType Directory -Force -Path (Split-Path -Parent $stage) | Out-Null;" ^
 "try { Copy-Item -LiteralPath $session -Destination $stage -Recurse -ErrorAction Stop; Get-ChildItem -LiteralPath $stage -Recurse -Filter *.png | Remove-Item -Force;" ^
 " Compress-Archive -LiteralPath $stage -DestinationPath $text -Force -ErrorAction Stop }" ^
 "catch { Write-Host ('  Could not make the smaller zip: ' + $_.Exception.Message) }" ^
 "finally { Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue };" ^
 "Write-Host ''; Write-Host ('  ' + $zip + ' is too big for GitHub (25 MB).');" ^
 "if (Test-Path -LiteralPath $text) { Write-Host ('  Attach this smaller one to issue #121: ' + $text); Write-Host '  and drag two or three of the screenshots into the comment as pictures.' }"
exit /b %ERRORLEVEL%
