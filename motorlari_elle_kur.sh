#!/bin/bash
# ==============================================================================
# Amorfly AI — Motorları Elle Kurma (Yedek/Garanti Seçeneği)
#
# NE ZAMAN KULLANILIR: Uygulama içindeki "Otomatik Kur" butonları bir
# sebeple başarısız olursa (link değişmiş, ağ engellenmiş, format
# değişmiş vb.) bu scripti çalıştır — aynı motorları terminalden indirip
# Amorfly AI'ın TAM OLARAK ARADIĞI klasörlere yerleştirir. Uygulamayı
# yeniden başlattığında Tanılama sekmesi bunları otomatik yeşil görür,
# ekstra bir ayar gerekmez.
#
# Güvenli: sadece kullanıcı klasörüne (~/.local/share/Amorfly AI/) yazar,
# sudo gerektiren adımlar (apt paketleri) ayrı ve açıkça işaretlenmiştir.
# ==============================================================================
set -uo pipefail

GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; NC='\033[0m'
ok()   { echo -e "${GREEN}✓${NC} $1"; }
warn() { echo -e "${YELLOW}⚠${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; }

BIN_DIR="$HOME/.local/share/Amorfly AI/bin"
mkdir -p "$BIN_DIR"

echo "=========================================="
echo " Amorfly AI — Motorları Elle Kurma"
echo " Hedef klasör: $BIN_DIR"
echo "=========================================="
echo ""

# ------------------------------------------------------------------
# 1) Ollama — resmi kurulum scripti (Amorfly'ın kendi indirmesinden
#    bağımsız, en güvenilir yöntem; sistem genelinde kurar ve
#    arka planda 'ollama serve' olarak otomatik başlatır)
# ------------------------------------------------------------------
echo "-- 1/5: Ollama --"
if command -v ollama &> /dev/null; then
    ok "Ollama zaten kurulu ($(command -v ollama))"
else
    if curl -fsSL https://ollama.com/install.sh | sh; then
        ok "Ollama kuruldu"
    else
        fail "Ollama kurulumu başarısız — internet bağlantını kontrol et ya da https://ollama.com/download adresinden elle indir"
    fi
fi
# Servis arka planda çalışmıyorsa başlat (kurulum scripti genelde
# systemd ile otomatik başlatır, bu sadece ek güvence)
if ! curl -s http://127.0.0.1:11434 &> /dev/null; then
    warn "Ollama servisi yanıt vermiyor, arka planda başlatmayı deniyorum..."
    nohup ollama serve > /dev/null 2>&1 &
    sleep 2
fi
echo ""

# ------------------------------------------------------------------
# 2) Piper TTS — GitHub'dan doğrudan, Amorfly'ın beklediği tam klasöre
# ------------------------------------------------------------------
echo "-- 2/5: Piper TTS --"
PIPER_DIR="$BIN_DIR/piper"
if [ -f "$PIPER_DIR/piper" ]; then
    ok "Piper zaten kurulu ($PIPER_DIR/piper)"
else
    mkdir -p "$PIPER_DIR"
    PIPER_URL=$(curl -s https://api.github.com/repos/rhasspy/piper/releases/latest \
        | grep "browser_download_url" \
        | grep -i "linux" | grep -i "x86_64" \
        | grep -o 'https://[^"]*' | head -n 1)
    if [ -z "$PIPER_URL" ]; then
        fail "Piper indirme linki bulunamadı (GitHub API'ye erişilemedi olabilir). Elle: github.com/rhasspy/piper/releases"
    else
        TMP_TAR="/tmp/piper_amorfly.tar.gz"
        if curl -fsSL "$PIPER_URL" -o "$TMP_TAR" && tar -xzf "$TMP_TAR" -C "$BIN_DIR"; then
            chmod +x "$PIPER_DIR/piper" 2>/dev/null
            rm -f "$TMP_TAR"
            ok "Piper kuruldu ($PIPER_DIR/piper)"
        else
            fail "Piper indirilemedi/açılamadı: $PIPER_URL"
        fi
    fi
fi
echo ""

# ------------------------------------------------------------------
# 3) Piper Türkçe ses modeli — Hugging Face'ten
# ------------------------------------------------------------------
echo "-- 3/5: Piper Türkçe ses modeli --"
VOICES_DIR="$BIN_DIR/piper-voices"
mkdir -p "$VOICES_DIR"
HF_BASE="https://huggingface.co/rhasspy/piper-voices/resolve/main/tr/tr_TR/dfki/medium"
if [ -f "$VOICES_DIR/tr_TR-dfki-medium.onnx" ]; then
    ok "Türkçe ses modeli zaten kurulu"
else
    ok1=false; ok2=false
    curl -fsSL "$HF_BASE/tr_TR-dfki-medium.onnx" -o "$VOICES_DIR/tr_TR-dfki-medium.onnx" && ok1=true
    curl -fsSL "$HF_BASE/tr_TR-dfki-medium.onnx.json" -o "$VOICES_DIR/tr_TR-dfki-medium.onnx.json" && ok2=true
    if [ "$ok1" = true ] && [ "$ok2" = true ]; then
        ok "Türkçe ses modeli indirildi"
    else
        fail "Ses modeli indirilemedi. Elle: huggingface.co/rhasspy/piper-voices/tree/main/tr/tr_TR/dfki/medium"
    fi
fi
echo ""

# ------------------------------------------------------------------
# 4) Video2X — GitHub'dan doğrudan AppImage
# ------------------------------------------------------------------
echo "-- 4/5: Video2X --"
V2X_PATH="$BIN_DIR/video2x.AppImage"
if [ -f "$V2X_PATH" ]; then
    ok "Video2X zaten kurulu ($V2X_PATH)"
else
    V2X_URL=$(curl -s https://api.github.com/repos/k4yt3x/video2x/releases/latest \
        | grep "browser_download_url" \
        | grep -i "x86_64" | grep -i "\.appimage" \
        | grep -o 'https://[^"]*' | head -n 1)
    if [ -z "$V2X_URL" ]; then
        fail "Video2X indirme linki bulunamadı. Elle: github.com/k4yt3x/video2x/releases"
    else
        if curl -fsSL "$V2X_URL" -o "$V2X_PATH"; then
            chmod +x "$V2X_PATH"
            ok "Video2X kuruldu ($V2X_PATH)"
        else
            fail "Video2X indirilemedi: $V2X_URL"
        fi
    fi
fi
echo ""

# ------------------------------------------------------------------
# 5) Sistem paketleri (sudo gerekiyor) — ffmpeg, zstd, OCR, belge araçları
# ------------------------------------------------------------------
echo "-- 5/5: Sistem paketleri (sudo şifre isteyecek) --"
echo "Kurulacak: ffmpeg zstd tesseract-ocr tesseract-ocr-tur poppler-utils pandoc libreoffice xdotool"
read -p "Şimdi kurulsun mu? [e/H] " yn
if [[ "$yn" =~ ^[Ee]$ ]]; then
    if sudo apt update && sudo apt install -y ffmpeg zstd tesseract-ocr tesseract-ocr-tur poppler-utils pandoc libreoffice xdotool; then
        ok "Sistem paketleri kuruldu"
    else
        fail "Bazı paketler kurulamadı — yukarıdaki apt çıktısına bak"
    fi
else
    warn "Atlandı. Daha sonra elle: sudo apt install -y ffmpeg zstd tesseract-ocr tesseract-ocr-tur poppler-utils pandoc libreoffice xdotool"
fi
echo ""

echo "=========================================="
echo " Bitti. Amorfly AI'ı (yeniden) aç ve"
echo " 'Tanılama' sekmesinden durumu kontrol et."
echo "=========================================="
