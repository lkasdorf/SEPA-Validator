@echo off
:: SEPA XML Validator - Starter
:: Startet das PowerShell-Tool ohne Execution-Policy-Probleme
powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "%~dp0SEPA-Validator.ps1"
