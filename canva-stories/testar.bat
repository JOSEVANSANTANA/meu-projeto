@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo Testando a conexao com o Canva...
python testar_conexao.py
pause >nul
