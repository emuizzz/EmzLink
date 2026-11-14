@echo off
title Build EmzLink
echo ====================================================
echo   Build EmzLink (Rust + Tauri 2)
echo ====================================================
where cargo >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Rust/Cargo not found. Install it from https://rustup.rs
    pause
    exit /b 1
)
echo Closing running EmzLink (it locks the old EmzLink.exe)...
taskkill /F /T /IM EmzLink.exe >nul 2>&1
rem build files (several GB) go to a temp folder and are deleted afterwards
set "CARGO_TARGET_DIR=%TEMP%\emzlink-build"
cd /d "%~dp0src-tauri"
cargo build --release
if %errorlevel% neq 0 (
    echo [ERROR] Build failed.
    if exist "%CARGO_TARGET_DIR%" rmdir /s /q "%CARGO_TARGET_DIR%"
    pause
    exit /b 1
)
if not exist "%CARGO_TARGET_DIR%\release\emzlink.exe" (
    echo [ERROR] target\release\emzlink.exe not found.
    pause
    exit /b 1
)
rem the new exe replaces the old one in the main folder (one level above "source")
copy /y "%CARGO_TARGET_DIR%\release\emzlink.exe" "..\..\EmzLink.exe"
if %errorlevel% neq 0 (
    echo [ERROR] Could not replace ..\..\EmzLink.exe - close EmzLink and try again.
    rmdir /s /q "%CARGO_TARGET_DIR%"
    pause
    exit /b 1
)
rmdir /s /q "%CARGO_TARGET_DIR%"
if exist "%~dp0src-tauri\target" rmdir /s /q "%~dp0src-tauri\target"
echo.
echo Done. New EmzLink.exe is in the main folder:
echo %~dp0..\EmzLink.exe
pause
