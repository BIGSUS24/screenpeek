@echo off
REM ============================================================
REM  START the Screen Streaming Service (and allow phone access)
REM  RIGHT-CLICK this file -> "Run as administrator"
REM ============================================================

net session >nul 2>&1
if %errorLevel% neq 0 (
    echo.
    echo   You must RIGHT-CLICK start.bat and choose
    echo   "Run as administrator".
    echo.
    pause
    exit /b 1
)

REM 1. Make sure the firewall lets your phone reach port 8080 (add only if missing).
netsh advfirewall firewall show rule name="Screen Stream 8080" >nul 2>&1
if errorlevel 1 (
    echo Opening firewall for port 8080...
    netsh advfirewall firewall add rule name="Screen Stream 8080" dir=in action=allow protocol=TCP localport=8080 >nul
)

REM 2. Make sure auto-restart is set (in case stop.bat cleared it).
sc failure SSService reset= 0 actions= restart/2000/restart/2000/restart/2000 >nul 2>&1

REM 3. Start the service.
echo Starting Screen Streaming Service...
sc start SSService

echo.
echo   Watch on THIS PC:   http://localhost:8080
echo   Watch on your PHONE: http://192.168.0.109:8080   (same Wi-Fi)
echo.
echo   If the phone still cannot connect, your PC's address may have changed -
echo   run "ipconfig" and use the Wi-Fi "IPv4 Address" with :8080
echo.
pause
