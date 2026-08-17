#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOCS="$ROOT/docs"
RAW="$DOCS/promo/raw"
OUT_EN="$DOCS/shots/en"
OUT_ZH="$DOCS/shots/zh"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
BASE_APP="http://localhost:1420"
BASE_DOCS="http://localhost:8765"
COMMON="--headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 --virtual-time-budget=6000"

mkdir -p "$RAW" "$OUT_EN" "$OUT_ZH"

if ! curl -sf "$BASE_APP/" >/dev/null 2>&1; then
  echo "Start the app dev server first: cd apps/desktop && npm run dev"
  exit 1
fi

echo "→ Raw app captures"
capture_raw() {
  local file="$1" w="$2" h="$3" url="$4"
  "$CHROME" $COMMON --window-size="$w,$h" --screenshot="$RAW/$file" "$url"
  echo "  $file"
}

capture_raw home.png 440 720 "$BASE_APP/?scene=home&lang=zh&theme=light"
capture_raw connecting.png 440 720 "$BASE_APP/?scene=connecting&lang=zh&theme=light"
capture_raw incoming.png 440 720 "$BASE_APP/?scene=incoming&lang=zh&theme=light"
capture_raw settings.png 440 720 "$BASE_APP/?scene=settings&lang=zh&theme=light"
capture_raw session.png 980 720 "$BASE_APP/?scene=session&lang=zh&theme=light"

DOC_PID=""
cleanup() { [[ -n "$DOC_PID" ]] && kill "$DOC_PID" 2>/dev/null || true; }
trap cleanup EXIT

if ! curl -sf "$BASE_DOCS/promo/index.html" >/dev/null 2>&1; then
  python3 -m http.server 8765 --directory "$DOCS" >/dev/null 2>&1 &
  DOC_PID=$!
  sleep 1
fi

echo "→ Promo cards (EN + ZH)"
capture_promo() {
  local scene="$1" lang="$2" out="$3" w="$4" h="$5"
  "$CHROME" $COMMON --window-size="$w,$h" --screenshot="$out" \
    "$BASE_DOCS/promo/index.html?scene=$scene&lang=$lang"
  echo "  $out"
}

for lang in en zh; do
  out_dir="$OUT_EN"
  [[ "$lang" == "zh" ]] && out_dir="$OUT_ZH"
  capture_promo home "$lang" "$out_dir/home.png" 880 1320
  capture_promo connecting "$lang" "$out_dir/connecting.png" 880 1320
  capture_promo incoming "$lang" "$out_dir/incoming.png" 880 1320
  capture_promo settings "$lang" "$out_dir/settings.png" 880 1320
  capture_promo session "$lang" "$out_dir/session.png" 1920 1080
done

echo "→ OG images"
"$CHROME" $COMMON --window-size=1200,630 --screenshot="$DOCS/og-en.png" "$BASE_DOCS/promo/og.html?lang=en"
"$CHROME" $COMMON --window-size=1200,630 --screenshot="$DOCS/og-zh.png" "$BASE_DOCS/promo/og.html?lang=zh"
echo "  og-en.png og-zh.png"

echo "Done."
