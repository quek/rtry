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
    del "%INSTALL_DLL%" 2>nul
    if exist "%INSTALL_DLL%" (
        echo [ERROR] DLL is locked. Checking processes:
        tasklist /m rtry_tsf.dll
        echo.
        echo Close the above processes and retry.
        pause
        exit /b 1
    )
    echo [OK] DLL removed: %INSTALL_DLL%
) else (
    echo [SKIP] DLL not found: %INSTALL_DLL%
)

echo.
echo Uninstall complete.
pause
