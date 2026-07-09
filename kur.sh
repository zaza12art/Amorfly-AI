#!/bin/bash
# ==============================================================================
# Amorfly AI — Tek Komutla Kurulum (GitHub Release üzerinden)
#
# KULLANIM (terminalden tek satır, kod indirip zip açmaya gerek yok):
#
#   curl -fsSL https://raw.githubusercontent.com/zaza12art/Amorfly-AI/main/kur.sh | bash
#
# Bu komut GitHub Actions'ın ürettiği EN GÜNCEL .AppImage'ı indirir,
# çalıştırılabilir yapar, gerçek ikonunu çıkarır, hem uygulama menüsüne
# hem masaüstüne kısayol ekler. İşin bitince "Amorfly AI" ikonuna çift
# tıklaman yeterli.
#
# GÜNCELLEME: Yeni bir sürüm çıktığında AYNI komutu tekrar çalıştır —
# üzerine indirir, kısayolu bozmaz.
# ==============================================================================
set -uo pipefail

REPO="zaza12art/Amorfly-AI"
RELEASE_URL="https://github.com/$REPO/releases/download/latest/Amorfly-AI.AppImage"

GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; NC='\033[0m'
ok()   { echo -e "${GREEN}✓${NC} $1"; }
warn() { echo -e "${YELLOW}⚠${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; }

APP_DIR="$HOME/.local/share/amorfly-app"
ICON_DIR="$HOME/.local/share/icons"
APPS_DIR="$HOME/.local/share/applications"
APPIMAGE_PATH="$APP_DIR/Amorfly-AI.AppImage"

mkdir -p "$APP_DIR" "$ICON_DIR" "$APPS_DIR"

echo "=========================================="
echo " Amorfly AI — Kurulum başlıyor"
echo "=========================================="
echo ""

echo "-- En güncel sürüm indiriliyor --"
if curl -fsSL "$RELEASE_URL" -o "$APPIMAGE_PATH.tmp"; then
    mv "$APPIMAGE_PATH.tmp" "$APPIMAGE_PATH"
    chmod +x "$APPIMAGE_PATH"
    ok "İndirildi: $APPIMAGE_PATH"
else
    rm -f "$APPIMAGE_PATH.tmp"
    fail "İndirme başarısız. Olası sebepler:"
    echo "   - Depo adı yanlış olabilir (şu an '$REPO' varsayılıyor)"
    echo "   - GitHub Actions henüz hiç başarılı derleme yapmamış olabilir"
    echo "   - İnternet bağlantısı yok"
    echo "   Elle indirme: https://github.com/$REPO/releases/latest"
    exit 1
fi
echo ""

echo "-- Uygulama ikonu çıkarılıyor --"
if [ ! -f "$ICON_DIR/amorfly-ai.png" ]; then
    TMP_EXTRACT=$(mktemp -d)
    (cd "$TMP_EXTRACT" && "$APPIMAGE_PATH" --appimage-extract > /dev/null 2>&1) || true
    EXTRACTED_ICON=$(find "$TMP_EXTRACT/squashfs-root" -maxdepth 1 -iname "*.png" 2>/dev/null | head -n 1)
    if [ -n "$EXTRACTED_ICON" ]; then
        cp -f "$EXTRACTED_ICON" "$ICON_DIR/amorfly-ai.png"
        ok "İkon çıkarıldı"
    else
        warn "İkon bulunamadı, varsayılan sistem ikonu kullanılacak"
    fi
    rm -rf "$TMP_EXTRACT"
else
    ok "İkon zaten mevcut"
fi
echo ""

echo "-- Kısayollar oluşturuluyor --"
ICON_LINE="Icon=application-x-executable"
[ -f "$ICON_DIR/amorfly-ai.png" ] && ICON_LINE="Icon=$ICON_DIR/amorfly-ai.png"

DESKTOP_CONTENT="[Desktop Entry]
Name=Amorfly AI
Comment=Yerel-öncelikli, bağımsız masaüstü AI asistanı
Exec=\"$APPIMAGE_PATH\"
$ICON_LINE
Terminal=false
Type=Application
Categories=Utility;Development;
"

echo "$DESKTOP_CONTENT" > "$APPS_DIR/amorfly-ai.desktop"
chmod +x "$APPS_DIR/amorfly-ai.desktop"
ok "Uygulama menüsüne eklendi"

DESKTOP_DIR=""
for candidate in "$HOME/Desktop" "$HOME/Masaüstü"; do
    [ -d "$candidate" ] && { DESKTOP_DIR="$candidate"; break; }
done

if [ -n "$DESKTOP_DIR" ]; then
    echo "$DESKTOP_CONTENT" > "$DESKTOP_DIR/amorfly-ai.desktop"
    chmod +x "$DESKTOP_DIR/amorfly-ai.desktop"
    ok "Masaüstüne kısayol eklendi: $DESKTOP_DIR/amorfly-ai.desktop"
fi
echo ""

echo "=========================================="
echo " Bitti! 'Amorfly AI' simgesine çift tıkla."
echo " (İlk açılışta dosya yöneticisi 'Güven/Trust'"
echo "  onayı isteyebilir — bu normal bir güvenlik adımıdır.)"
echo ""
echo " Güncellemek için: bu komutu tekrar çalıştırman yeterli."
echo "=========================================="
