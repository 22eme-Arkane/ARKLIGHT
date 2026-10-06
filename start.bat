@echo off
rem ARKLIGHT - lance le simulateur et ouvre l'interface dans le navigateur.
cd /d "%~dp0"
if not exist node_modules (
  echo Installation des dependances...
  call npm install || goto :erreur
)
start "" cmd /c "timeout /t 2 /nobreak >nul & start http://localhost:8686"
node server\index.js
goto :eof

:erreur
echo.
echo L'installation a echoue : Node.js est-il installe ? (https://nodejs.org)
pause
