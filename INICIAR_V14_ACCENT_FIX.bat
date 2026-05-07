@echo off
title Dublador Master Pro v14.1 - Anti-Corte Final
cd /d "%~dp0"

:: Ambiente local do projeto
set VENV_PYTHON=%~dp0.venv\Scripts\python.exe
set FFMPEG_DIR=%~dp0.venv\ffmpeg
set PATH=%FFMPEG_DIR%;%PATH%

echo ============================================================
echo  Dublador Master Pro v14.1 - Anti-Sotaque + Anti-Corte Final
echo  Venv: %VENV_PYTHON%
echo  FFmpeg: %FFMPEG_DIR%\ffmpeg.exe
echo ============================================================
echo.

:: Verificar se o venv existe
if not exist "%VENV_PYTHON%" (
    echo [ERRO] Venv nao encontrado em:
    echo    %VENV_PYTHON%
    echo.
    echo Execute a preparacao do ambiente antes de iniciar.
    pause
    exit /b 1
)

if not exist "%FFMPEG_DIR%\ffmpeg.exe" (
    echo [ERRO] FFmpeg nao encontrado em:
    echo    %FFMPEG_DIR%\ffmpeg.exe
    pause
    exit /b 1
)

:: Mostrar versao do python e torch antes de iniciar
echo Verificando ambiente...
"%VENV_PYTHON%" -c "import torch; print('  torch:', torch.__version__, '| CUDA:', torch.cuda.is_available())" 2>nul
if errorlevel 1 (
    echo  [ERRO] torch nao disponivel neste venv
    pause
    exit /b 1
)
echo.
echo Iniciando programa...
echo.

"%VENV_PYTHON%" DUBLAGEM_MASTER_PRO_v14_ACCENT_FIX.py

echo.
echo ============================================================
echo  Programa encerrado (codigo: %errorlevel%)
echo  Se houve erro, verifique: crash_log.txt
echo ============================================================
pause
