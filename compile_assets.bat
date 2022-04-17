@echo off
setlocal

cd money-web
wasm-pack build -t web --release
cd ..\

cp money-web/pkg/money_web_bg.wasm static
del static\web.js
call npx spack
