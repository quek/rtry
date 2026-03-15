@echo off
setlocal

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Run as Administrator.
    pause
    exit /b 1
)

set INSTALL_DLL=%ProgramFiles%\rtry\rtry_tsf.dll

if exist "%INSTALL_DLL%" (
    regsvr32 /u /s "%INSTALL_DLL%"
    if %errorlevel% equ 0 (
        echo [OK] DLL unregistered.
    ) else (
        echo [ERROR] DLL unregistration failed.
    )
    del "%INSTALL_DLL%"
    echo [OK] DLL removed: %INSTALL_DLL%
) else (
    echo [SKIP] DLL not found: %INSTALL_DLL%
)

if exist "%ProgramFiles%\rtry\debug.log" (
    del "%ProgramFiles%\rtry\debug.log"
)

echo.
echo Uninstall complete.
pause
