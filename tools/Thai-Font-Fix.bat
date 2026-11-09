@echo off
title Thai Font Fix
echo.
echo  Setting Thai text in drawings to use Cordia New...
echo.
setx CADCRAFT_FONTALT "Cordia" >nul
setx CADCRAFT_FONTFALLBACK "Tahoma" >nul
if %errorlevel%==0 (
  echo  Done! Close the CAD program completely, then open it again.
) else (
  echo  Something went wrong. Please try again.
)
echo.
pause
