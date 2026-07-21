@echo off
REM ============================================================
REM  Rode este arquivo UMA VEZ, no primeiro uso, para instalar
REM  as dependencias (a biblioteca Pillow).
REM ============================================================
chcp 65001 >nul
cd /d "%~dp0"
echo Instalando dependencias (Pillow)...
python -m pip install --upgrade pip
python -m pip install -r requirements.txt
echo.
echo Pronto! Ja pode usar o gerar_banner.bat
echo Pressione qualquer tecla para fechar...
pause >nul
