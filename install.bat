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

:: Copy DLL and data files to Program Files (accessible by AppContainer)
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /y "%BUILD_DLL%" "%INSTALL_DLL%" >nul
echo [OK] DLL copied to %INSTALL_DLL%
copy /y "%TABLE_SRC%" "%INSTALL_DIR%\try.tbl" >nul
echo [OK] try.tbl copied to %INSTALL_DIR%\try.tbl
if exist "%DIC_SRC%" (
    copy /y "%DIC_SRC%" "%INSTALL_DIR%\mazegaki.dic" >nul
    echo [OK] mazegaki.dic copied to %INSTALL_DIR%\mazegaki.dic
)

:: Register DLL from install location
regsvr32 /s "%INSTALL_DLL%"
if %errorlevel% equ 0 (
    echo [OK] DLL registered: %INSTALL_DLL%
) else (
    echo [ERROR] DLL registration failed.
)

pause
