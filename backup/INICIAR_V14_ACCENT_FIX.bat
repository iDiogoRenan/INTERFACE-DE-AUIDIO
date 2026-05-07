@echo off
title Dublador Master Pro v14.1 - Anti-Corte Final
cd /d "%~dp0"

:: Python do venv correto (torch128 RTX 50)
set VENV_PYTHON=D:\CD DUBLAGEM PROJETO\venv\Scripts\python.exe

echo ============================================================
echo  Dublador Master Pro v14.1 - Anti-Sotaque + Anti-Corte Final
echo  Venv: %VENV_PYTHON%
echo ============================================================
echo.

:: Verificar se o venv existe
if not exist "%VENV_PYTHON%" (
    echo [ERRO] Venv nao encontrado em:
    echo    %VENV_PYTHON%
    echo.
    echo Verifique se a pasta D:\CD DUBLAGEM PROJETO\venv existe.
    pause
    exit /b 1
)

:: Mostrar versao do python e torch antes de iniciar
echo Verificando ambiente...
"%VENV_PYTHON%" -c "import torch; print('  torch:', torch.__version__, '| CUDA:', torch.cuda.is_available())" 2>nul
if errorlevel 1 (
    echo  [AVISO] torch nao disponivel neste venv
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
