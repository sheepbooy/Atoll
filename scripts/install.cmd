@echo off
REM Install Atoll from the latest GitHub Release (Windows).
REM Works in cmd.exe, PowerShell, and Windows Terminal.
REM
REM Usage:
REM   curl -fsSL https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/install.cmd -o install.cmd && install.cmd
REM
REM Pin a version (cmd):
REM   set ATOLL_VERSION=0.1.11 && install.cmd

powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/install.ps1 | iex"
