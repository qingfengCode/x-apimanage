@echo off
REM Run cargo check with MSVC env (see build-msvc.cmd for why)
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
if errorlevel 1 (
  echo [ERROR] Failed to load MSVC env
  exit /b 1
)
set "CC_x86_64_pc_windows_msvc=cl.exe"
set "CXX_x86_64_pc_windows_msvc=cl.exe"
set "PATH=%PATH:C:\mypc\TDM-GCC-64\bin;=%"
cd /d "%~dp0"
echo === cargo check ===
call cargo check 2>&1
echo === done, exit code %errorlevel% ===
