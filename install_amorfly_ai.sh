#!/bin/bash
# ==============================================================================
# Amorfly AI — Build, Bağımlılık Kontrolü ve Masaüstü Kısayolu (hepsi bir arada)
# Ubuntu, Debian, Zorin OS, Pop!_OS
# ==============================================================================
set -e

GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; NC='\033[0m'
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo -e "${GREEN}== Amorfly AI kurulum/derleme betiği ==${NC}"

if [ "$EUID" -eq 0 ]; then
    echo -e "${RED}Lütfen bu betiği sudo ile ÇALIŞTIRMAYIN.${NC} Gerektiğinde kendisi şifre soracak."
    exit 1
fi

check_cmd() {
    if command -v "$1" &> /dev/null; then
        echo -e "${GREEN}✓${NC} $1 kurulu"
        return 0
    else
        echo -e "${YELLOW}✗${NC} $1 bulunamadı"
        return 1
    fi
}

echo ""
echo "-- Zorunlu bağımlılıklar --"
NEED_APT=false
check_cmd node || NEED_APT=true
check_cmd npm || NEED_APT=true
check_cmd cargo || { echo "  Rust kurulu değil. Kurmak için: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; }

echo ""
echo "-- Tauri sistem kütüphaneleri --"
dpkg -s libwebkit2gtk-4.1-dev &> /dev/null && echo -e "${GREEN}✓${NC} libwebkit2gtk-4.1-dev" || { echo -e "${YELLOW}✗${NC} libwebkit2gtk-4.1-dev eksik"; NEED_APT=true; }

if [ "$NEED_APT" = true ]; then
    echo ""
    echo -e "${YELLOW}Eksik sistem paketlerini kurmak için:${NC}"
    echo "sudo apt update && sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev nodejs npm"
fi

echo ""
echo "-- Opsiyonel: Altyazı/Dublaj/Belge özellikleri için --"
check_cmd ollama || echo "  Kurulum: uygulama içinden Tanılama sekmesi ile otomatik, ya da https://ollama.com"
check_cmd ffmpeg || echo "  Kurulum: sudo apt install ffmpeg"
check_cmd zstd || echo "  Kurulum: sudo apt install zstd (Ollama'nın otomatik kurulumu için gerekli)"
check_cmd xdotool || echo "  Kurulum: sudo apt install xdotool (kullanım alışkanlığı takibi için, X11)"
check_cmd whisper-cli || echo "  Kurulum: github.com/ggerganov/whisper.cpp (derleyip PATH'e ekleyin)"
check_cmd piper || echo "  Kurulum: uygulama içinden Tanılama sekmesi ile otomatik, ya da github.com/rhasspy/piper"
check_cmd video2x || echo "  Kurulum: uygulama içinden Kalite Artırma sekmesi ile otomatik, ya da github.com/k4yt3x/video2x"
check_cmd pdftotext || echo "  Kurulum: sudo apt install poppler-utils (PDF okuma için)"
check_cmd pandoc || echo "  Kurulum: sudo apt install pandoc (Word okuma + .docx/.pdf üretimi için)"
check_cmd libreoffice || echo "  Kurulum: sudo apt install libreoffice (.pdf üretimi için)"
check_cmd tesseract || echo "  Kurulum: sudo apt install tesseract-ocr tesseract-ocr-tur (toplu OCR için)"

echo ""
echo "İpucu: Kurulum sonrası uygulamayı açtığında 'Tanılama' sekmesi tüm bu araçları"
echo "otomatik tarar ve eksik olanlar için tek tık kurulum/terminal butonu sunar."

# ------------------------------------------------------------------
# Masaüstü kısayolu / uygulama menüsü girişi oluşturma (build sonrası
# otomatik çalışır — ayrı bir script çalıştırmana gerek yok)
# ------------------------------------------------------------------
create_desktop_shortcut() {
    local appimage="$1"

    # Elle yol verilmediyse, olası konumları sırayla ara:
    # 1) Bu projede yerel derleme çıktısı
    # 2) GitHub Actions'tan indirip Masaüstüne/İndirilenler'e koyduğun AppImage
    if [ -z "$appimage" ]; then
        appimage=$(find "$PROJECT_DIR/src-tauri/target/release/bundle/appimage" -maxdepth 1 -iname "*.AppImage" 2>/dev/null | head -n 1)
    fi
    if [ -z "$appimage" ]; then
        for dir in "$HOME/Desktop" "$HOME/Masaüstü" "$HOME/Downloads" "$HOME/İndirilenler"; do
            found=$(find "$dir" -maxdepth 1 -iname "*amorfly*.AppImage" 2>/dev/null | head -n 1)
            if [ -n "$found" ]; then appimage="$found"; break; fi
        done
    fi

    if [ -z "$appimage" ]; then
        echo -e "${YELLOW}AppImage hiçbir yerde bulunamadı.${NC}"
        echo "Elle belirtmek için: ./install_amorfly_ai.sh --shortcut-only \"/tam/yol/dosya.AppImage\""
        return 1
    fi

    echo "Bulunan AppImage: $appimage"

    local app_home="$HOME/.local/share/amorfly-app"
    local icon_home="$HOME/.local/share/icons"
    local apps_dir="$HOME/.local/share/applications"
    mkdir -p "$app_home" "$icon_home" "$apps_dir"

    cp -f "$appimage" "$app_home/Amorfly-AI.AppImage"
    chmod +x "$app_home/Amorfly-AI.AppImage"

    # Gerçek logo: önce proje kaynağından dene, yoksa AppImage'ın kendi
    # içinden --appimage-extract ile çıkar (proje klasörü elde değilse,
    # ör. sadece indirilen AppImage varsa bu devreye girer).
    if [ -f "$PROJECT_DIR/src-tauri/icons/icon.png" ]; then
        cp -f "$PROJECT_DIR/src-tauri/icons/icon.png" "$icon_home/amorfly-ai.png"
    elif [ ! -f "$icon_home/amorfly-ai.png" ]; then
        local tmp_extract
        tmp_extract=$(mktemp -d)
        (cd "$tmp_extract" && "$appimage" --appimage-extract > /dev/null 2>&1) || true
        local extracted_icon
        extracted_icon=$(find "$tmp_extract/squashfs-root" -maxdepth 1 -iname "*.png" 2>/dev/null | head -n 1)
        if [ -n "$extracted_icon" ]; then
            cp -f "$extracted_icon" "$icon_home/amorfly-ai.png"
        fi
        rm -rf "$tmp_extract"
    fi

    local icon_line="Icon=application-x-executable"
    if [ -f "$icon_home/amorfly-ai.png" ]; then
        icon_line="Icon=$icon_home/amorfly-ai.png"
    fi

    local desktop_content="[Desktop Entry]
Name=Amorfly AI
Comment=Yerel-öncelikli, bağımsız masaüstü AI asistanı
Exec=\"$app_home/Amorfly-AI.AppImage\"
$icon_line
Terminal=false
Type=Application
Categories=Utility;Development;
"

    echo "$desktop_content" > "$apps_dir/amorfly-ai.desktop"
    chmod +x "$apps_dir/amorfly-ai.desktop"
    echo -e "${GREEN}✓${NC} Uygulama menüsüne eklendi: $apps_dir/amorfly-ai.desktop"

    local desktop_dir=""
    for candidate in "$HOME/Desktop" "$HOME/Masaüstü"; do
        if [ -d "$candidate" ]; then desktop_dir="$candidate"; break; fi
    done

    if [ -n "$desktop_dir" ]; then
        echo "$desktop_content" > "$desktop_dir/amorfly-ai.desktop"
        chmod +x "$desktop_dir/amorfly-ai.desktop"
        echo -e "${GREEN}✓${NC} Masaüstüne kısayol eklendi: $desktop_dir/amorfly-ai.desktop"
        echo "  (İlk çift tıklamada dosya yöneticisi 'Güven/Trust' onayı isteyebilir — bu normal)"
    fi

    echo -e "${GREEN}Tamamlandı.${NC} Uygulama menüsünden 'Amorfly' araması yaparak da açabilirsin."
}

# --shortcut-only modu: derleme yapmadan, sadece kısayol oluştur
# (GitHub Actions'tan AppImage indirdiysen bunu kullan)
if [ "$1" = "--shortcut-only" ]; then
    create_desktop_shortcut "$2"
    exit 0
fi

echo ""
read -p "Şimdi 'npm install' ve Tauri build (.deb + .AppImage) çalıştırılsın mı? [e/H] " yn
if [[ "$yn" =~ ^[Ee]$ ]]; then
    npm install
    npm run tauri:build
    echo -e "${GREEN}Derleme tamamlandı.${NC} Çıktılar: src-tauri/target/release/bundle/"
    echo ""
    echo "-- Masaüstü kısayolu oluşturuluyor --"
    create_desktop_shortcut || echo -e "${YELLOW}Kısayol otomatik oluşturulamadı, elle kurman gerekebilir.${NC}"
else
    echo "Daha sonra manuel çalıştırmak için: npm install && npm run tauri:build"
    echo "Build bittikten sonra kısayol oluşturmak için: ./install_amorfly_ai.sh --shortcut-only"
fi
