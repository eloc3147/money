@echo off
setlocal

del /Q assets
copy static assets
call npx spack
