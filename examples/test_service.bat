@echo off
echo [%DATE% %TIME%] Test batch service started
echo [%DATE% %TIME%] Working directory: %CD%
echo [%DATE% %TIME%] Arguments: %*

set COUNTER=0

:loop
set /A COUNTER+=1
echo [%DATE% %TIME%] Batch service heartbeat #%COUNTER%

if "%1"=="exit" (
    if %COUNTER% GEQ 3 (
        echo [%DATE% %TIME%] Programmed exit after 3 heartbeats
        goto end
    )
)

timeout /t 3 /nobreak >nul
goto loop

:end
echo [%DATE% %TIME%] Batch service ending normally
