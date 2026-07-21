@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo Instalando dependencias...
python -m pip install --upgrade pip
python -m pip install -r requirements.txt
echo.
echo Pronto! Agora rode o iniciar.bat para abrir o programa.
pause >nul
