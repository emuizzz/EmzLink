@echo off
title EmzLink - reset settings
echo ====================================================
echo   EmzLink: reset saved settings and cache
echo ====================================================
echo.
echo Stopping EmzLink and zapret...
taskkill /f /im EmzLink.exe >nul 2>&1
taskkill /f /im winws.exe >nul 2>&1
echo Removing autostart tasks...
schtasks /Delete /TN EmzLinkApp /F >nul 2>&1
schtasks /Delete /TN EmzLinkBypass /F >nul 2>&1
schtasks /Delete /TN EmzLinkTgProxy /F >nul 2>&1
echo Removing Telegram proxy files and old autostart entries...
taskkill /f /im TgWsProxy.exe >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v EmzLink /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v TgWsProxy /f >nul 2>&1
if exist "%~dp0tgproxy" rd /s /q "%~dp0tgproxy" >nul 2>&1
if exist "%APPDATA%\TgWsProxy" rd /s /q "%APPDATA%\TgWsProxy" >nul 2>&1
sc stop WinDivert >nul 2>&1
echo Removing settings and cache...
if exist "%APPDATA%\com.emzlink.app" rd /s /q "%APPDATA%\com.emzlink.app" >nul 2>&1
if exist "%LOCALAPPDATA%\com.emzlink.app" rd /s /q "%LOCALAPPDATA%\com.emzlink.app" >nul 2>&1
echo Resetting lists and game/ipset modes in this folder...
echo domain.example.abc>"%~dp0zapret\lists\list-general-user.txt"
echo 203.0.113.113/32>"%~dp0zapret\lists\ipset-all.txt"
del /q "%~dp0zapret\lists\emz-list-general.txt" "%~dp0zapret\lists\emz-list-google.txt" "%~dp0zapret\utils\game_filter.enabled" >nul 2>&1
echo.
echo Done. Start EmzLink.exe again.
echo.
pause
