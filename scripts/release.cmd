@echo off
rem ClipCat release wrapper for Windows: forwards to scripts/release.sh
rem using the bash that ships with Git for Windows.
rem Usage: scripts\release.cmd <patch|minor|major|X.Y.Z|verify> [options]

setlocal
set "BASH=bash.exe"
where /q bash.exe
if errorlevel 1 (
    if exist "%ProgramFiles%\Git\bin\bash.exe" (
        set "BASH=%ProgramFiles%\Git\bin\bash.exe"
    ) else if exist "%LocalAppData%\Programs\Git\bin\bash.exe" (
        set "BASH=%LocalAppData%\Programs\Git\bin\bash.exe"
    ) else (
        echo release: error: bash.exe not found - install Git for Windows. 1>&2
        exit /b 1
    )
)
"%BASH%" "%~dp0release.sh" %*
exit /b %errorlevel%
