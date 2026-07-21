@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo Instalando dependencias (requests)...
python -m pip install --upgrade pip
python -m pip install -r requirements.txt
echo.
echo Pronto! Proximo passo: rode o autorizar.bat (apenas uma vez).
pause >nul
