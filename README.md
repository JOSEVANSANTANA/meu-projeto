# ESF News Monitor

Aplicação desktop local (Windows) de alta performance para monitoramento de
notícias financeiras em tempo real com predição de impacto no **S&P 500
Futuro (ES)**, construída com **Tauri (Rust) + React/TypeScript + Tailwind**,
análise via **Google Gemini** e histórico em **SQLite embutido**.

> ⚠️ Uso estritamente informativo/educacional. Não é recomendação de
> investimento. Respeite os Termos de Uso das fontes monitoradas.

---

## Arquitetura (3 pilares)

```
┌────────────────────────────── TAURI (processo único) ──────────────────────────────┐
│                                                                                     │
│  PILAR 1 · INGESTÃO (Rust/tokio)          PILAR 2 · PREDIÇÃO          PILAR 3 · UI  │
│  ┌─────────────────────────┐      ┌──────────────────────────┐   ┌───────────────┐  │
│  │ investing.rs (3★ only)  │      │ gemini.rs                │   │ React + TW    │  │
│  │ financial_juice.rs      │─────▶│ system_prompt rígido     │──▶│ Dashboard     │  │
│  │ + polite_delay 2–5s     │      │ responseSchema JSON      │   │ estilo        │  │
│  │ + UA de navegador       │      │ retry se schema inválido │   │ Bloomberg     │  │
│  └─────────────────────────┘      └──────────────────────────┘   └───────┬───────┘  │
│              │                                │                          │          │
│              ▼                                ▼                          ▼          │
│        dedup (SHA-256)                 SQLite (WAL)              Notificação nativa │
│                                        esf_news.db              + alerta sonoro    │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

## Estrutura de pastas

```
esf-news-monitor/
├── .env                        # SEGREDOS locais (não versionado)
├── .env.example                # modelo de configuração
├── index.html
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── src/                        # ─── FRONTEND (React + TS) ───
│   ├── main.tsx
│   ├── App.tsx                 # Dashboard principal
│   ├── types.ts                # espelho TS dos structs Rust
│   ├── styles.css
│   ├── hooks/
│   │   └── useNewsStream.ts    # histórico + stream Tauri + som de alerta
│   └── components/
│       ├── NewsCard.tsx        # card colorido por sentimento
│       └── StatusBar.tsx       # status do motor + relógios
└── src-tauri/                  # ─── BACKEND (Rust) ───
    ├── Cargo.toml
    ├── tauri.conf.json         # janela, CSP, bundle Windows
    ├── capabilities/
    │   └── default.json        # permissões (eventos + notificações)
    └── src/
        ├── main.rs
        ├── lib.rs              # bootstrap: .env, SQLite, loop, commands
        ├── engine.rs           # loop: scrape → Gemini → DB → UI → alerta
        ├── models.rs           # RawNewsItem, GeminiAnalysis, NewsEvent
        ├── gemini.rs           # cliente Gemini c/ saída JSON estrita
        ├── db.rs               # SQLite (rusqlite, WAL)
        └── scrapers/
            ├── mod.rs          # anti-bot: delays 2–5s + headers reais
            ├── investing.rs    # calendário econômico (só 3 estrelas, EUA)
            └── financial_juice.rs  # manchetes + Truth Social (RSS)
```

## Pré-requisitos (Windows)

1. **Rust** — https://rustup.rs (instala `cargo`)
2. **Node.js 20+** — https://nodejs.org
3. **Microsoft Visual Studio C++ Build Tools** (exigido pelo Rust no Windows)
4. **WebView2** — já vem no Windows 10/11 atualizados

## Configuração do `.env` (chave do Gemini)

1. Gere sua chave em **https://aistudio.google.com/apikey** (login Google →
   *Create API key*).
2. Na **raiz do projeto**, copie o modelo e edite:

   ```powershell
   copy .env.example .env
   ```

3. Preencha:

   ```dotenv
   GEMINI_API_KEY=AIza...sua_chave_aqui
   GEMINI_MODEL=gemini-2.5-flash
   SCRAPE_INTERVAL_SECS=30
   MIN_REQUEST_DELAY_MS=2000
   MAX_REQUEST_DELAY_MS=5000
   # Opcional: espelho RSS para posts do Truth Social
   # TRUTH_SOCIAL_RSS_URL=https://...
   ```

**Segurança da chave:**
- O `.env` fica **apenas na sua máquina** e já está no `.gitignore` — nunca é
  commitado.
- A chave é lida **somente pelo processo Rust** (`dotenvy` em `lib.rs`);
  ela nunca chega ao webview/frontend, então não há como vazá-la via UI.
- Se a chave vazar, revogue-a no AI Studio e gere outra.

## Rodando

```powershell
npm install

# Gera os ícones do app (necessário 1x antes do primeiro build)
# Use qualquer PNG quadrado 1024x1024 como origem:
npx tauri icon caminho/para/icone.png

# Desenvolvimento (hot-reload)
npm run tauri dev

# Build de produção (gera .msi / instalador NSIS)
npm run tauri build
```

## Notas de engenharia

- **Anti-bot**: todo request de scraping passa por `polite_delay()`
  (2–5 s aleatórios) + User-Agent de navegador real + cookies + jitter no
  intervalo do ciclo. Ajuste os limites no `.env` se necessário.
- **Custo do Gemini**: a deduplicação (SHA-256 da manchete+dado) acontece
  **antes** da chamada à API — cada notícia é analisada uma única vez.
- **Saída estrita do LLM**: dupla garantia — `system_instruction` rígida +
  `responseSchema` na `generationConfig`; respostas fora do schema são
  descartadas e re-tentadas (até 3x).
- **Seletores de scraping**: sites de terceiros mudam o HTML sem aviso.
  Se um conector logar `0 itens` por vários ciclos, revise os seletores em
  `src-tauri/src/scrapers/`.
- **RAM**: o backend é 100% Rust (sem Chromium embutido como no Electron);
  consumo típico < 150 MB.
