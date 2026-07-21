@echo off
REM ============================================================
REM  Duplo clique aqui para abrir o gerador de banners.
REM  Ele abre o modo interativo (pergunta dia, nome e foto).
REM ============================================================
chcp 65001 >nul
cd /d "%~dp0"
echo Iniciando o gerador de banners...
python gerar_banner.py
echo.
echo Pressione qualquer tecla para fechar...
pause >nul
