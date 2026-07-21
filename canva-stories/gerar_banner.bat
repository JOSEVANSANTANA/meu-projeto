@echo off
chcp 65001 >nul
cd /d "%~dp0"
python gerar_banner.py
echo.
echo Pressione qualquer tecla para fechar...
pause >nul
