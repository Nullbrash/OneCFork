#!/usr/bin/env bash
# Публикация собранного релиза на GitHub (метод — глобальное правило
# «Публикация релизов на GitHub»). Только по явной команде пользователя.
#   bash scripts/publish-release.sh notes.txt     (описание — файлом, UTF-8)
#   bash scripts/publish-release.sh "что нового"
# Ключ доступа берётся из Git Credential Manager и живёт только в переменной
# этого процесса — не выводится и никуда не пишется.
set -euo pipefail
cd "$(dirname "$0")/.."

OWNER="Nullbrash"
REPO="OneCFork"
EXPECTED_ACCOUNT="Nullbrash"

VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
TAG="v$VERSION"
DIR="src-tauri/target/release/bundle/nsis"
EXE="OneCFork_${VERSION}_x64-setup.exe"
for f in "$EXE" "$EXE.sig" "latest.json"; do
  [ -f "$DIR/$f" ] || { echo "нет файла $DIR/$f — сначала build-release.ps1" >&2; exit 1; }
done
PRERELEASE=false
case "$VERSION" in *-*) PRERELEASE=true ;; esac
# Описание — из файла, если передан путь: кириллица в аргументах программ
# Windows из Git Bash искажается системной кодовой страницей.
# Временные файлы — по относительному пути: «@/tmp/…» в аргументе curl
# Git Bash не переводит в путь Windows.
NOTES_FILE="$DIR/.release-notes.txt"
BODY_FILE="$DIR/.release-body.json"
trap 'rm -f "$NOTES_FILE" "$BODY_FILE"' EXIT
if [ -n "${1:-}" ] && [ -f "$1" ]; then cp "$1" "$NOTES_FILE"; else printf '%s' "${1:-}" > "$NOTES_FILE"; fi

export GCM_INTERACTIVE=never
TOKEN=$(printf "protocol=https\nhost=github.com\n\n" | git credential fill 2>/dev/null | sed -n 's/^password=//p')
[ -n "$TOKEN" ] || { echo "нет разрешения GitHub в Git Credential Manager" >&2; exit 1; }
api() { curl -sS -H "Authorization: token $TOKEN" -H "Accept: application/vnd.github+json" "$@"; }

# Аккаунт — по API, а не по полю username из Git Credential Manager.
LOGIN=$(api https://api.github.com/user | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>console.log(JSON.parse(s).login||''))")
[ "$LOGIN" = "$EXPECTED_ACCOUNT" ] || { echo "ключ принадлежит '$LOGIN', ожидался '$EXPECTED_ACCOUNT' — стоп" >&2; exit 1; }
CAN_PUSH=$(api "https://api.github.com/repos/$OWNER/$REPO" | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>console.log(!!(JSON.parse(s).permissions||{}).push))")
[ "$CAN_PUSH" = "true" ] || { echo "у $LOGIN нет прав на запись в $OWNER/$REPO — стоп" >&2; exit 1; }

# Повторный запуск не должен создать второй релиз с тем же тегом.
EXISTS=$(api -o /dev/null -w "%{http_code}" "https://api.github.com/repos/$OWNER/$REPO/releases/tags/$TAG")
[ "$EXISTS" = "404" ] || { echo "релиз $TAG уже есть (HTTP $EXISTS) — стоп" >&2; exit 1; }

# Тело запроса — файлом в UTF-8 (через аргумент curl кириллица ломается).
node -e "const fs=require('fs');fs.writeFileSync(process.argv[1],JSON.stringify({tag_name:process.argv[2],target_commitish:'main',name:'OneCFork '+process.argv[3],body:fs.readFileSync(process.argv[4],'utf8').trim(),draft:false,prerelease:process.argv[5]==='true'}),'utf8')" "$BODY_FILE" "$TAG" "$VERSION" "$NOTES_FILE" "$PRERELEASE"
RELEASE=$(api -X POST -H "Content-Type: application/json; charset=utf-8" "https://api.github.com/repos/$OWNER/$REPO/releases" --data-binary "@$BODY_FILE")
ID=$(echo "$RELEASE" | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{const r=JSON.parse(s);if(!r.id){console.error(r.message||s);process.exit(1)}console.log(r.id)})")
UPLOAD="https://uploads.github.com/repos/$OWNER/$REPO/releases/$ID/assets"
for f in "$EXE" "$EXE.sig" "latest.json"; do
  api -X POST -H "Content-Type: application/octet-stream" --data-binary "@$DIR/$f" "$UPLOAD?name=$f" \
    | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{const a=JSON.parse(s);if(!a.id){console.error(a.message||s);process.exit(1)}console.log('загружено:',a.name,a.size,'байт')})"
done
unset TOKEN
echo "релиз $TAG опубликован (prerelease=$PRERELEASE): https://github.com/$OWNER/$REPO/releases/tag/$TAG"
