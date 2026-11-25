@echo off
setlocal EnableExtensions
chcp 65001 >nul
title Daybook server

rem Always run relative to this file, not the caller's folder.
cd /d "%~dp0"

rem ---------------------------------------------------------------------------
rem Optional overrides (set these before running, or uncomment/edit):
rem
rem   set "DAYBOOK_BIND=0.0.0.0:8787"
rem   set "DAYBOOK_DATA_DIR=C:\Daybook\data"
rem   set "DAYBOOK_ADMIN_PASSWORD=..."    (first run only)
rem   set "DAYBOOK_NO_TLS=1"               (force plain HTTP even if certs exist)
rem
rem If certs\daybook-cert.pem and certs\daybook-key.pem exist, HTTPS is used
rem automatically. Generate them with:
rem   daybook.exe cert init --host <LAN-IP>,localhost --out certs
rem ---------------------------------------------------------------------------

if not defined DAYBOOK_BIND set "DAYBOOK_BIND=0.0.0.0:8787"

rem Prefer the newest published release in releases\; fall back to a local build.
set "RELEASES=%~dp0releases"
set "EXE="
if exist "%RELEASES%\daybook-v*.exe" (
    for /f "usebackq delims=" %%F in (`powershell -NoProfile -Command "$d=Get-ChildItem -LiteralPath $env:RELEASES -Filter 'daybook-v*.exe' -File | Sort-Object {[version](($_.Name -replace '^daybook-v','') -replace '\.exe$','')} -Descending; if($d){$d[0].FullName}"`) do set "EXE=%%F"
)

if not defined EXE set "EXE=%~dp0target\release\daybook.exe"
if not exist "%EXE%" set "EXE=%~dp0target\debug\daybook.exe"
if not exist "%EXE%" (
    echo Building Daybook release binary... this can take a few minutes.
    cargo build --release
    if errorlevel 1 goto :error
    set "EXE=%~dp0target\release\daybook.exe"
)

if defined DAYBOOK_NO_TLS (
    rem Explicitly force plain HTTP, even if certificates are configured.
    set "DAYBOOK_TLS_CERT="
    set "DAYBOOK_TLS_KEY="
) else (
    if not defined DAYBOOK_TLS_CERT if exist "%~dp0certs\daybook-cert.pem" (
        set "DAYBOOK_TLS_CERT=%~dp0certs\daybook-cert.pem"
    )
    if not defined DAYBOOK_TLS_KEY if exist "%~dp0certs\daybook-key.pem" (
        set "DAYBOOK_TLS_KEY=%~dp0certs\daybook-key.pem"
    )
)

set "SCHEME=http"
if defined DAYBOOK_TLS_CERT set "SCHEME=https"
if defined DAYBOOK_TLS_CERT if not defined DAYBOOK_TLS_KEY (
    echo.
    echo ERROR: DAYBOOK_TLS_CERT is set but DAYBOOK_TLS_KEY is missing.
    echo Both files are required for HTTPS, or set DAYBOOK_NO_TLS=1 for HTTP.
    goto :error
)
if defined DAYBOOK_TLS_KEY if not defined DAYBOOK_TLS_CERT (
    echo.
    echo ERROR: DAYBOOK_TLS_KEY is set but DAYBOOK_TLS_CERT is missing.
    echo Both files are required for HTTPS, or set DAYBOOK_NO_TLS=1 for HTTP.
    goto :error
)

rem Pull the port out of the bind address for the friendly URL (IPv4 form).
set "PORT=8787"
for /f "tokens=2 delims=:" %%P in ("%DAYBOOK_BIND%") do set "PORT=%%P"
if not defined PORT set "PORT=8787"

echo.
echo ============================================================
echo   Daybook
echo   bind      : %DAYBOOK_BIND%
echo   protocol  : %SCHEME%
echo   build     : %EXE%
if defined DAYBOOK_DATA_DIR echo   data dir  : %DAYBOOK_DATA_DIR%
if not defined DAYBOOK_DATA_DIR echo   data dir  : platform default (%%LOCALAPPDATA%%\Daybook)
echo   this PC   : %SCHEME%://localhost:%PORT%
echo   phone/LAN : %SCHEME%://^<this-PC-LAN-IP^>:%PORT%
if "%SCHEME%"=="http" (
    echo.
    echo   Note: PWA install/offline mode needs HTTPS. Generate certs with
    echo   "daybook.exe cert init --host ^<LAN-IP^>,localhost --out certs"
    echo   and restart this script.
)
echo.
echo   Press Ctrl+C to stop the server.
echo ============================================================
echo.

"%EXE%" serve
set "EXITCODE=%ERRORLEVEL%"

echo.
if not "%EXITCODE%"=="0" (
    echo Daybook stopped with an error ^(exit code %EXITCODE%^).
    echo Check the messages above. The database and data directory are safe.
) else (
    echo Daybook stopped.
)
echo.
pause
endlocal & exit /b %EXITCODE%

:error
echo.
echo Daybook could not start.
pause
endlocal & exit /b 1
