# Amorfly AI

Gizlilik odaklı, yerel-öncelikli (offline-first), bağımsız Linux masaüstü AI asistanı.

- **Google/Gemini yok.** Varsayılan ve tek zorunlu motor: yerel Ollama.
- **PWA değil.** Tauri (Rust + native WebView) ile gerçek masaüstü uygulaması.
- **Sahte cevap yok.** Yerel model çalışmıyorsa uygulama bunu açıkça söyler, uydurmaz.

Mevcut durum ve yapılacaklar listesi için: [`docs/architecture.md`](docs/architecture.md)

## En kolay yol: Tek terminal komutuyla kurulum (zip yok, Codespace yok)

İlk kurulumdan sonra proje GitHub Actions'ta zaten derlenmiş ve bir "Release" olarak
yayınlanmışsa, terminalden tek komutla indirip masaüstüne kurabilirsin:

```bash
curl -fsSL https://raw.githubusercontent.com/zaza12art/Amorfly-AI/main/kur.sh | bash
```

Bu, en güncel `.AppImage`'ı indirir, gerçek ikonuyla hem uygulama menüsüne hem masaüstüne
kısayol ekler. Yeni bir sürüm çıktığında **aynı komutu tekrar çalıştırman** yeterli.

## Kendi bilgisayarında HİÇBİR ŞEY kurmadan derle (ilk kurulum / GitHub tarafı)

Rust/npm kurmak istemiyorsan, derlemeyi GitHub'ın ücretsiz sunucularına yaptırabilirsin:

1. Bu klasörü bir GitHub deposuna yükle (git veya GitHub Desktop ile).
2. Depoda **Actions** sekmesine gir.
3. **"Amorfly AI - Masaüstü Uygulaması Derle"** workflow'unu seç, **"Run workflow"**'a bas.
4. ~5-10 dakika sonra iş bitince, aynı sayfadaki **Artifacts** bölümünden
   `amorfly-ai-linux.zip`'i indir — içinde hazır `.AppImage` ve `.deb` var.
5. `.AppImage`'ı `chmod +x` yapıp çift tıkla, hazır.

Bu yöntemde kendi terminalinde Rust/npm kurmana, derleme hatasıyla uğraşmana gerek kalmaz —
tüm hatalar (varsa) GitHub'ın loglarında görünür, ben ya da sen oradan takip edip düzeltebiliriz.

## Alternatif: Kendi Linux makinende derleme

```bash
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

npm install
npm run tauri:dev
```

Ollama kurulu ve `ollama serve` ile çalışıyor olmalı: https://ollama.com
