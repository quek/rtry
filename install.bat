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
set TABLE_SRC=%~dp0data\try.tbl
set TABLE_DST=%APPDATA%\rtry\try.tbl

if not exist "%DLL_PATH%" (
    echo [ERROR] DLL not found: %DLL_PATH%
    echo Run: cargo build --release -p rtry-tsf
    pause
    exit /b 1
)

if not exist "%APPDATA%\rtry" mkdir "%APPDATA%\rtry"
copy /y "%TABLE_SRC%" "%TABLE_DST%" >nul
echo [OK] try.tbl copied to %TABLE_DST%

regsvr32 /s "%DLL_PATH%"
if %errorlevel% equ 0 (
    echo [OK] DLL registered: %DLL_PATH%
    echo.
    echo Add "Try-Code" keyboard in:
    echo   Settings - Time and Language - Language - Japanese - Keyboard
) else (
    echo [ERROR] DLL registration failed.
)

pause
