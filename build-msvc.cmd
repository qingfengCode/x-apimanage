@echo off
REM ============================================================================
REM  Windows build helper for x-apimanage
REM
REM  Reason: TDM-GCC (C:\mypc\TDM-GCC-64\bin\gcc.exe) sits in PATH.
REM  The Rust cc crate picks it up to build libsqlite3-sys, producing MinGW's
REM  __chkstk_ms symbol; Tauri's MSVC link.exe then fails with LNK2019.
REM
REM  This script loads VS MSVC x64 env, removes TDM-GCC from PATH, forces
REM  CC=cl.exe, then runs tauri build. Use this instead of direct
REM  `npm run tauri build`.
REM
REM  Usage: double-click, or  cmd /c build-msvc.cmd
REM  For dev hot-reload: change the last line to `npx tauri dev`
REM ============================================================================

call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
if errorlevel 1 (
  echo [ERROR] Failed to load MSVC env. Make sure VS 2022 with C++ workload is installed.
  exit /b 1
)

set "CC_x86_64_pc_windows_msvc=cl.exe"
set "CXX_x86_64_pc_windows_msvc=cl.exe"

REM Remove TDM-GCC from PATH if present
set "PATH=%PATH:C:\mypc\TDM-GCC-64\bin;=%"

cd /d "%~dp0"
echo === starting tauri build ===
call npx tauri build
echo === done, exit code %errorlevel% ===
