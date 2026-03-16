@echo off
setlocal

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Run as Administrator.
    pause
    exit /b 1
)

set BUILD_DLL=%~dp0target\release\rtry_tsf.dll
set INSTALL_DIR=%ProgramFiles%\rtry
set INSTALL_DLL=%INSTALL_DIR%\rtry_tsf.dll
set TABLE_SRC=%~dp0data\try.tbl
set DIC_SRC=%~dp0data\mazegaki.dic

if not exist "%BUILD_DLL%" (
    echo [ERROR] DLL not found: %BUILD_DLL%
    echo Run: cargo build --release -p rtry-tsf
    pause
    exit /b 1
)

if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

:: Check if DLL is locked by another process
copy /y "%BUILD_DLL%" "%INSTALL_DLL%" >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] DLL is locked. Checking processes:
    tasklist /m rtry_tsf.dll
    echo.
    echo Close the above processes and retry.
    pause
    exit /b 1
)
echo [OK] DLL copied to %INSTALL_DLL%

copy /y "%TABLE_SRC%" "%INSTALL_DIR%\try.tbl" >nul
echo [OK] try.tbl copied to %INSTALL_DIR%\try.tbl
if exist "%DIC_SRC%" (
    copy /y "%DIC_SRC%" "%INSTALL_DIR%\mazegaki.dic" >nul
    echo [OK] mazegaki.dic copied to %INSTALL_DIR%\mazegaki.dic
)

:: Copy config.json if it exists (for AppContainer apps like Claude)
set CONFIG_SRC=%APPDATA%\rtry\config.json
if exist "%CONFIG_SRC%" (
    copy /y "%CONFIG_SRC%" "%INSTALL_DIR%\config.json" >nul
    echo [OK] config.json copied to %INSTALL_DIR%\config.json
)

:: Copy config tool
set BUILD_CONFIG=%~dp0target\release\rtry-config.exe
if exist "%BUILD_CONFIG%" (
    copy /y "%BUILD_CONFIG%" "%INSTALL_DIR%\rtry-config.exe" >nul
    echo [OK] rtry-config.exe copied to %INSTALL_DIR%\rtry-config.exe
)

regsvr32 /s "%INSTALL_DLL%"
if %errorlevel% equ 0 (
    echo [OK] DLL registered: %INSTALL_DLL%
) else (
    echo [ERROR] DLL registration failed.
)

pause
