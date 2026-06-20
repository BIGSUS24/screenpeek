@echo off
setlocal

REM Run from the script's own directory (admin launches default to System32)
cd /d "%~dp0"

echo ============================================
echo  Screen Streaming Service - Installer
echo ============================================
echo.
echo Project folder: %CD%
echo.

REM --- Require administrator -------------------------------------------------
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo ERROR: This script requires administrator privileges.
    echo Right-click install.bat and select "Run as administrator".
    pause
    exit /b 1
)

REM --- Require cargo ---------------------------------------------------------
where cargo >nul 2>&1
if %errorLevel% neq 0 (
    echo ERROR: Rust/Cargo not found. Install from https://rustup.rs then reopen the terminal.
    pause
    exit /b 1
)

if not exist "Cargo.toml" (
    echo ERROR: Cargo.toml not found in %CD%. Keep install.bat in the project root.
    pause
    exit /b 1
)

echo [1/6] Building release binaries (first build can take a few minutes)...
cargo build --release --bin ss-service --bin ss-cli
if %errorLevel% neq 0 (
    echo ERROR: Build failed.
    pause
    exit /b 1
)

echo [2/6] Stopping/removing any previous service...
REM Clear auto-restart first so killing the old process doesn't respawn it,
REM then stop + force-kill so the .exe is unlocked before we copy over it.
sc config SSService start= disabled >nul 2>&1
sc failure SSService reset= 0 actions= "" >nul 2>&1
sc stop SSService >nul 2>&1
timeout /t 2 /nobreak >nul
taskkill /F /IM ss-service.exe >nul 2>&1
timeout /t 1 /nobreak >nul
sc delete SSService >nul 2>&1
timeout /t 1 /nobreak >nul

echo [3/6] Copying binaries to ProgramData...
set INSTALL_DIR=%ProgramData%\ss-service
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /Y "target\release\ss-service.exe" "%INSTALL_DIR%\" >nul
copy /Y "target\release\ss-cli.exe" "%INSTALL_DIR%\" >nul
copy /Y "viewer\index.html" "%INSTALL_DIR%\" >nul

echo [4/6] Creating config + consent (if needed)...
if not exist "%INSTALL_DIR%\config.toml" (
    "%INSTALL_DIR%\ss-cli.exe" setup
)
REM Capture requires consent; grant it now (answer y).
"%INSTALL_DIR%\ss-cli.exe" consent grant

echo [5/6] Registering Windows service...
REM start= demand  -> YOU start it when you want (change to "auto" for start-at-boot)
REM obj= LocalSystem -> runs as SYSTEM so it can capture the login/lock/secure desktop
sc create SSService binPath= "%INSTALL_DIR%\ss-service.exe" start= demand obj= LocalSystem DisplayName= "Screen Streaming Service"
if %errorLevel% neq 0 (
    echo ERROR: Service creation failed.
    pause
    exit /b 1
)
sc description SSService "Personal screen streaming service. Captures the desktop (incl. lock/login screen) and serves it on http://localhost:8080."

echo [6/6] Setting auto-restart (so nothing but you can stop it)...
REM If the process is ever killed, Windows restarts it. reset= 0 means the
REM restart policy never gives up. A deliberate Stop (services.msc / sc stop)
REM does NOT trigger a restart - that is how YOU stop it.
sc failure SSService reset= 0 actions= restart/2000/restart/2000/restart/2000

echo.
echo ============================================
echo  Installation Complete!
echo ============================================
echo.
echo START it now:        sc start SSService
echo Watch on this PC:    http://localhost:8080
echo Watch on phone:      http://YOUR-PC-IP:8080   (same Wi-Fi; open firewall - see HOW-TO-RUN.md)
echo.
echo STOP it (only you can):
echo   Task Manager  -^> Services tab -^> right-click SSService -^> Stop
echo   or:  sc stop SSService     (run as admin)
echo   NOTE: "End task" in the Processes tab will NOT stop it - it auto-restarts.
echo.
echo Remove it completely:  uninstall.bat  (as administrator)
echo.
pause
