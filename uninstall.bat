@echo off
setlocal

:: Admin check
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Run as Administrator.
    pause
    exit /b 1
)

set DLL_PATH=%~dp0target\release\rtry_tsf.dll

if exist "%DLL_PATH%" (
    regsvr32 /u /s "%DLL_PATH%"
    if %errorlevel% equ 0 (
        echo [OK] DLL unregistered.
    ) else (
        echo [ERROR] DLL unregistration failed.
    )
) else (
    echo [SKIP] DLL not found: %DLL_PATH%
)

if exist "%APPDATA%\rtry" (
    rmdir /s /q "%APPDATA%\rtry"
    echo [OK] Removed %APPDATA%\rtry
)

echo.
echo Uninstall complete.
pause
