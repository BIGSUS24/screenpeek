@echo off
REM ============================================================
REM  STOP the Screen Streaming Service + ALL its processes
REM  RIGHT-CLICK this file -> "Run as administrator"
REM ============================================================

net session >nul 2>&1
if %errorLevel% neq 0 (
    echo.
    echo   You must RIGHT-CLICK stop.bat and choose
    echo   "Run as administrator".
    echo.
    pause
    exit /b 1
)

echo Stopping Screen Streaming Service and all its processes...
echo.

REM 1. Clear the auto-restart policy so force-killing does not respawn it.
sc failure SSService reset= 0 actions= "" >nul 2>&1

REM 2. Ask the service manager to stop it (may be ignored by a stuck build).
sc stop SSService >nul 2>&1
timeout /t 2 /nobreak >nul

REM 3. Force-kill EVERY ss-service.exe instance (the service in Session 0 AND the
REM    capture agent in your session - both share this exe name). /T also kills
REM    any child processes. Repeat a few times to catch anything mid-relaunch.
for /L %%i in (1,1,3) do (
    taskkill /F /T /IM ss-service.exe >nul 2>&1
    timeout /t 1 /nobreak >nul
)

REM 4. Verify nothing is left. ("if errorlevel 1" is evaluated at runtime, so it
REM    avoids the %errorLevel% parse-time expansion bug inside blocks.)
tasklist /FI "IMAGENAME eq ss-service.exe" 2>nul | find /I "ss-service.exe" >nul
if errorlevel 1 (
    echo   Done - all processes stopped.
) else (
    echo   WARNING: still running - one more try...
    taskkill /F /T /IM ss-service.exe >nul 2>&1
    timeout /t 2 /nobreak >nul
    tasklist /FI "IMAGENAME eq ss-service.exe" 2>nul | find /I "ss-service.exe" >nul
    if errorlevel 1 (
        echo   Done - all processes stopped.
    ) else (
        echo   STILL RUNNING - please reboot the PC to clear it.
    )
)

echo   (The web page will no longer show your screen.)
echo.
pause
