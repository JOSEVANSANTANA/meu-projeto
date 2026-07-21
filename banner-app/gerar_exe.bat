@echo off
REM ============================================================
REM  Gera o executavel (.exe) do programa com PyInstaller.
REM  Rode isto UMA vez (ou quando mudar o codigo).
REM  O .exe final fica na pasta "dist".
REM ============================================================
chcp 65001 >nul
cd /d "%~dp0"
echo Instalando o PyInstaller (se necessario)...
python -m pip install --upgrade pyinstaller
echo.
echo Gerando o executavel...
python -m PyInstaller --noconfirm --onefile --windowed --name "GeradorDeBanners" app.py
echo.
echo ============================================================
echo  Pronto! O executavel esta em:  dist\GeradorDeBanners.exe
echo.
echo  IMPORTANTE: copie estas pastas para o lado do .exe (mesma
echo  pasta que o GeradorDeBanners.exe):
echo     config\        (templates.json)
echo     templates_bg\  (os fundos PNG)
echo     fontes\        (a fonte .ttf)
echo     fotos\  e  saida\
echo ============================================================
pause >nul
