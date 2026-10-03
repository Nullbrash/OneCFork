#!/usr/bin/env bash
# Публикация собранного релиза на GitHub (метод — глобальное правило
# «Публикация релизов на GitHub»). Только по явной команде пользователя.
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
NOTES="${1:-}"

export GCM_INTERACTIVE=never
TOKEN=$(printf "protocol=https\nhost=github.com\n\n" | git credential fill 2>/dev/null | sed -n 's/^password=//p')
[ -n "$TOKEN" ] || { echo "нет разрешения GitHub в Git Credential Manager" >&2; exit 1; }
api() { curl -sS -H "Authorization: token $TOKEN" -H "Accept: application/vnd.github+json" "$@"; }

# Аккаунт — по API, а не по полю username из Git Credential Manager.
LOGIN=$(api https://api.github.com/user | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>console.log(JSON.parse(s).login||''))")
[ "$LOGIN" = "$EXPECTED_ACCOUNT" ] || { echo "ключ принадлежит '$LOGIN', ожидался '$EXPECTED_ACCOUNT' — стоп" >&2; exit 1; }
CAN_PUSH=$(api "https://api.github.com/repos/$OWNER/$REPO" | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>console.log(!!(JSON.parse(s).permissions||{}).push))")
[ "$CAN_PUSH" = "true" ] || { echo "у $LOGIN нет прав на запись в $OWNER/$REPO — стоп" >&2; exit 1; }

BODY=$(node -e "console.log(JSON.stringify({tag_name:process.argv[1],target_commitish:'main',name:'OneCFork '+process.argv[2],body:process.argv[3],draft:false,prerelease:process.argv[4]==='true'}))" "$TAG" "$VERSION" "$NOTES" "$PRERELEASE")
RELEASE=$(api -X POST "https://api.github.com/repos/$OWNER/$REPO/releases" -d "$BODY")
ID=$(echo "$RELEASE" | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{const r=JSON.parse(s);if(!r.id){console.error(r.message||s);process.exit(1)}console.log(r.id)})")
UPLOAD="https://uploads.github.com/repos/$OWNER/$REPO/releases/$ID/assets"
for f in "$EXE" "$EXE.sig" "latest.json"; do
  api -X POST -H "Content-Type: application/octet-stream" --data-binary "@$DIR/$f" "$UPLOAD?name=$f" \
    | node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{const a=JSON.parse(s);if(!a.id){console.error(a.message||s);process.exit(1)}console.log('загружено:',a.name,a.size,'байт')})"
done
unset TOKEN
echo "релиз $TAG опубликован (prerelease=$PRERELEASE): https://github.com/$OWNER/$REPO/releases/tag/$TAG"
