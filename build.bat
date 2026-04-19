@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64 >nul 2>&1
cd /d G:\Local-Projects\Claude-Cowork-Project\Claude-Copilot-Node-Memory\app\src-tauri
C:\Users\kate-\.cargo\bin\cargo.exe check 2>&1
echo EXIT_CODE=%ERRORLEVEL%
