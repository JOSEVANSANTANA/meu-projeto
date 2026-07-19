# ESF News Monitor no iPhone — push 24/7 + App Store

Arquitetura do produto (a intenção do projeto):

```
┌──────── SERVIDOR NA NUVEM (24/7, independente do seu Mac) ────────┐        ┌─── iPhone (App Store) ───┐
│ server/ : ingestão RSS + Trump → Gemini → SQLite → API HTTP       │  APNs  │ App: dashboard (lê a API) │
│           + push APNs p/ TODOS os aparelhos registrados           │ ─────▶ │ + push nativo (bloqueado, │
└───────────────────────────────────────────────────────────────────┘        │   no bolso, sempre)       │
                                                                             └──────────────────────────┘
```

O Mac é só a **bancada de desenvolvimento** (compilar, assinar, publicar).
Uma vez publicado, tudo roda sem ele: o servidor analisa e empurra os alertas,
e qualquer pessoa que baixar o app recebe.

> ✅ O servidor (`server/`) já está PRONTO e foi TESTADO: compila, sobe a API
> (`/health`, `/events`, `/devices`), roda a ingestão e envia push (em DRY-RUN
> até você configurar as credenciais da Apple).

---

## Custos — a verdade antes de começar

| Item | Custo |
|---|---|
| Servidor (Oracle Cloud Always Free) | **R$ 0** (VM ARM 24/7 grátis de verdade) |
| Chave Gemini (nível gratuito) | **R$ 0** |
| Rodar o app **no SEU iPhone** (conta Apple grátis) | **R$ 0** (build expira em 7 dias, reinstala pelo Xcode) |
| **App Store / TestFlight / push APNs** | **US$ 99/ano** (Apple Developer Program — não tem como fugir; é exigência da Apple para publicar e para usar push) |

Ou seja: dá para **desenvolver e testar tudo de graça**; para **publicar na App
Store com push**, a assinatura da Apple é obrigatória.

---

## PARTE 1 — Servidor 24/7 grátis (Oracle Cloud Always Free)

1. Crie a conta em https://www.oracle.com/cloud/free/ (Always Free).
2. Crie uma VM **Ampere A1 (ARM)** — até 4 OCPUs/24 GB grátis — com Ubuntu 22.04.
3. Na VM (via `ssh ubuntu@IP_DA_VM`):

```bash
# dependências
sudo apt update && sudo apt install -y build-essential pkg-config git python3
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# projeto
git clone <SEU_REPO> esf && cd esf/server
cargo build --release

# configuração (crie o .env na pasta server/)
cat > .env <<'EOF'
GEMINI_API_KEY=SUA_CHAVE_GEMINI
PRIORITY_ASSET=S&P 500 Futuro (ES)
PORT=8080
# APNs (preencher na PARTE 3; sem isso o push fica em DRY-RUN)
# APNS_TEAM_ID=
# APNS_KEY_ID=
# APNS_P8_PATH=/home/ubuntu/AuthKey_XXXX.p8
# APNS_TOPIC=com.esf.newsmonitor
# APNS_SANDBOX=1
EOF

# roda como serviço (systemd) — sobrevive a reboot
sudo tee /etc/systemd/system/esf.service > /dev/null <<'EOF'
[Unit]
Description=ESF News Monitor Server
After=network.target
[Service]
WorkingDirectory=/home/ubuntu/esf/server
ExecStart=/home/ubuntu/esf/server/target/release/esf-server
Restart=always
User=ubuntu
[Install]
WantedBy=multi-user.target
EOF
sudo systemctl enable --now esf
curl http://localhost:8080/health   # deve responder: ok
```

4. Libere a porta 8080 no Security List da VM (ou melhor: coloque um Caddy/
   nginx com HTTPS na frente — o iOS exige **HTTPS** para o app falar com a API;
   com Caddy é automático: `sudo apt install caddy` e um `Caddyfile` com
   `seu-dominio.com { reverse_proxy localhost:8080 }`; um subdomínio grátis do
   DuckDNS resolve se você não tiver domínio).

*Alternativas ao Oracle:* Google Cloud `e2-micro` (always free, EUA) ou um VPS
barato (~US$4/mês). Evite os "free tiers" que hibernam (Render/Railway) — um
monitor de pregão não pode dormir.

---

## PARTE 2 — App iOS no seu iPhone (no Mac, pelo terminal)

Pré-requisitos (uma vez): Xcode instalado (App Store), depois:

```bash
xcode-select --install
brew install cocoapods
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
```

No projeto:

```bash
npm install
npm run tauri ios init          # gera src-tauri/gen/apple (projeto Xcode)
npm run tauri ios dev           # roda no SIMULADOR para validar
```

Para rodar **no seu iPhone físico**:

```bash
npm run tauri ios dev --open    # abre o Xcode
```
No Xcode: alvo do app → **Signing & Capabilities** → marque *Automatically
manage signing* → selecione seu **Team** (seu Apple ID) → escolha o iPhone no
seletor de dispositivos → **▶ Run**. No iPhone: Ajustes → Geral →
Gerenciamento de VPN e Dispositivo → confie no seu certificado.

---

## PARTE 3 — Push nativo (APNs)

1. Com a conta Apple Developer paga: portal → *Certificates, IDs & Profiles* →
   **Keys** → crie uma chave com **Apple Push Notifications service (APNs)** →
   baixe o `AuthKey_XXXX.p8` (guarde! só baixa uma vez) e anote **Key ID** e
   **Team ID**.
2. No Xcode: alvo do app → *Signing & Capabilities* → **+ Capability** →
   **Push Notifications** (e *Background Modes → Remote notifications*).
3. No projeto Xcode gerado (`src-tauri/gen/apple/`), adicione ao
   `AppDelegate.swift` (ou arquivo equivalente de bootstrap) o registro de push:

```swift
import UserNotifications

// dentro de application(_:didFinishLaunchingWithOptions:):
UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { granted, _ in
    if granted { DispatchQueue.main.async { UIApplication.shared.registerForRemoteNotifications() } }
}

// callbacks (na classe do AppDelegate):
func application(_ application: UIApplication,
                 didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
    let token = deviceToken.map { String(format: "%02x", $0) }.joined()
    // registra o aparelho no SEU servidor:
    var req = URLRequest(url: URL(string: "https://SEU-SERVIDOR/devices")!)
    req.httpMethod = "POST"
    req.setValue("application/json", forHTTPHeaderField: "Content-Type")
    req.httpBody = try? JSONSerialization.data(withJSONObject: ["token": token])
    URLSession.shared.dataTask(with: req).resume()
}

func application(_ application: UIApplication,
                 didFailToRegisterForRemoteNotificationsWithError error: Error) {
    print("APNs register error: \(error)")
}
```

4. No servidor, preencha o `.env` (PARTE 1) com `APNS_TEAM_ID`, `APNS_KEY_ID`,
   `APNS_P8_PATH` (envie o .p8 para a VM com `scp`), `APNS_TOPIC` (bundle id) e
   `APNS_SANDBOX=1` para builds de desenvolvimento. Reinicie: `sudo systemctl restart esf`.
5. Teste: abra o app no iPhone (ele registra o token) e aguarde um evento
   CRITICAL/HIGH — o push chega com o iPhone bloqueado. Para forçar um teste,
   confira `curl https://SEU-SERVIDOR/devices/count` (deve ser ≥1) e acompanhe
   `journalctl -u esf -f` na VM.

---

## PARTE 4 — Publicar (TestFlight → App Store)

```bash
npm run tauri ios build         # gera o .ipa assinado (Team selecionado no Xcode)
```

1. **App Store Connect** (https://appstoreconnect.apple.com) → *My Apps* → **+**
   → registre o app com o mesmo bundle id (`com.esf.newsmonitor`).
2. Envie o build: no Xcode (*Product → Archive → Distribute*) ou com
   `xcrun altool`/Transporter.
3. **TestFlight**: disponível minutos depois — convide testadores por e-mail/link
   (até 10.000 pessoas). É a forma de "outras pessoas baixarem" antes da loja.
4. **App Store**: preencha ficha (descrição, screenshots, privacidade) e envie
   para revisão (1–3 dias). ⚠️ Aviso honesto de revisão: apps financeiros com
   "predições" precisam de disclaimer claro (já temos no rodapé) e a Apple pode
   pedir ajustes — é normal na primeira submissão. Troque `APNS_SANDBOX` para
   `0` no servidor quando o app for de produção/TestFlight externo.

---

## O que foi verificado aqui vs. o que depende do seu Mac/conta

- ✔ **Servidor**: compila, API testada (health/eventos/registro de aparelho com
  validação), ingestão rodando, push DRY-RUN operante. Pronto para deploy.
- ✔ **Base do app iOS**: código preparado (partes desktop-only isoladas).
- ⚠️ **Só no seu Mac/conta Apple**: `tauri ios init/build`, assinatura, o snippet
  Swift no projeto gerado, APNs real e a publicação — a Apple exige Xcode e a
  conta de desenvolvedor; nenhum ambiente Linux faz isso.
