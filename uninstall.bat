@echo off
setlocal

cd /d "%~dp0"

echo ============================================
echo  Screen Sharing Service Uninstaller
echo ============================================
echo.

REM Check for administrator privileges
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo ERROR: This script requires administrator privileges.
    echo Right-click and select "Run as administrator".
    pause
    exit /b 1
)

echo [1/4] Disabling auto-restart so it does not respawn during removal...
sc config SSService start= disabled >nul 2>&1
sc failure SSService reset= 0 actions= "" >nul 2>&1

echo [2/4] Stopping service and capture agent...
sc stop SSService >nul 2>&1
timeout /t 2 /nobreak >nul
REM Force-kill any lingering service/agent process (recovery is cleared above).
taskkill /F /IM ss-service.exe >nul 2>&1
timeout /t 1 /nobreak >nul

echo [3/4] Deleting service...
sc delete SSService >nul 2>&1

echo [4/4] Removing files...
set INSTALL_DIR=%ProgramData%\ss-service
if exist "%INSTALL_DIR%" (
    rmdir /s /q "%INSTALL_DIR%"
    echo Removed: %INSTALL_DIR%
)

REM Remove registry entries if any
reg delete "HKLM\SYSTEM\CurrentControlSet\Services\SSService" /f >nul 2>&1

echo.
echo ============================================
echo  Uninstall Complete!
echo ============================================
echo.
echo The service has been removed.
echo Configuration files have been deleted.
echo.
pause
