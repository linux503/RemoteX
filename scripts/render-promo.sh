#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOCS="$ROOT/docs"
OUT_EN="$DOCS/shots/en"
OUT_ZH="$DOCS/shots/zh"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
BASE_APP="http://localhost:1420"
BASE_DOCS="http://localhost:8765"
COMMON="--headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 --virtual-time-budget=6000"

mkdir -p "$OUT_EN" "$OUT_ZH"

if ! curl -sf "$BASE_APP/" >/dev/null 2>&1; then
  echo "Start the app dev server first: cd apps/desktop && npm run dev"
  exit 1
fi

echo "→ App screenshots (EN + ZH)"
capture() {
  local out="$1" w="$2" h="$3" url="$4"
  "$CHROME" $COMMON --window-size="$w,$h" --screenshot="$out" "$url"
  echo "  $out"
}

for lang in en zh; do
  out_dir="$OUT_EN"
  [[ "$lang" == "zh" ]] && out_dir="$OUT_ZH"
  capture "$out_dir/home.png" 900 700 "$BASE_APP/?scene=home&lang=$lang&theme=light"
  capture "$out_dir/connecting.png" 900 700 "$BASE_APP/?scene=connecting&lang=$lang&theme=light"
  capture "$out_dir/incoming.png" 900 700 "$BASE_APP/?scene=incoming&lang=$lang&theme=light"
  capture "$out_dir/settings.png" 900 700 "$BASE_APP/?scene=settings&tab=general&lang=$lang&theme=light"
  capture "$out_dir/session.png" 1120 760 "$BASE_APP/?scene=session&lang=$lang&theme=dark"
done

DOC_PID=""
cleanup() { [[ -n "$DOC_PID" ]] && kill "$DOC_PID" 2>/dev/null || true; }
trap cleanup EXIT

if ! curl -sf "$BASE_DOCS/promo/og.html" >/dev/null 2>&1; then
  python3 -m http.server 8765 --directory "$DOCS" >/dev/null 2>&1 &
  DOC_PID=$!
  sleep 1
fi

echo "→ OG images"
"$CHROME" $COMMON --window-size=1200,630 --screenshot="$DOCS/og-en.png" "$BASE_DOCS/promo/og.html?lang=en"
"$CHROME" $COMMON --window-size=1200,630 --screenshot="$DOCS/og-zh.png" "$BASE_DOCS/promo/og.html?lang=zh"
echo "  og-en.png og-zh.png"

echo "Done."
