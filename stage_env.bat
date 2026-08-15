@echo off
set BUILD_TOOLS=C:\Programs\BuildTools

if not exist "%BUILD_TOOLS%" (
  echo BUILD_TOOLS not found at: "%BUILD_TOOLS%"
  exit /b 1
)

call "%BUILD_TOOLS%\devcmd.bat"

@echo on
