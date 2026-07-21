@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo Abrindo autorizacao com o Canva...
python autorizar.py
pause >nul
