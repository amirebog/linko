@echo off
setlocal
cd /d "%~dp0"

where cargo >nul 2>nul
if errorlevel 1 (
    echo Cargo was not found.
    echo Install Rust from https://rustup.rs/ then reopen this terminal.
    echo Typical command: winget install Rustlang.Rustup
    pause
    exit /b 1
)

if not exist ".env" (
    if exist ".env.example" (
        copy /Y ".env.example" ".env" >nul
        echo Created .env from .env.example
    )
)

if not exist "data" mkdir data

echo Starting Linko...
cargo run
if errorlevel 1 (
    echo.
    echo Linko failed to start. See the messages above.
    pause
    exit /b 1
)

endlocal
