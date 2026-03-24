@echo off
:: Kopiert die XSD-Schemas aus dem Repository in den lokalen schemas-Ordner
set "SCRIPT_DIR=%~dp0"
set "SCHEMA_SRC=%SCRIPT_DIR%..\xml_schema"
set "SCHEMA_DST=%SCRIPT_DIR%schemas"

if not exist "%SCHEMA_SRC%" (
    echo FEHLER: Schema-Quellordner nicht gefunden: %SCHEMA_SRC%
    echo Bitte stellen Sie sicher, dass sich dieses Script im windows\-Ordner des Repositories befindet.
    pause
    exit /b 1
)

if not exist "%SCHEMA_DST%" mkdir "%SCHEMA_DST%"

echo Kopiere XSD-Schemas...
copy /Y "%SCHEMA_SRC%\*.xsd" "%SCHEMA_DST%\" >nul
echo Fertig. %SCHEMA_DST% enthaelt jetzt:
dir /B "%SCHEMA_DST%\*.xsd"
echo.
echo Sie koennen SEPA-Validator.cmd jetzt starten.
pause
