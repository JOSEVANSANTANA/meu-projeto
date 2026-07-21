# Gerador de Banners para Stories — Integração Oficial com o Canva

Programa de linha de comando (CMD do Windows) que gera banners **usando 100% a
Canva Connect API**: ele envia a foto, preenche um Brand Template seu (Nome,
Data, Horário e Imagem) via **Autofill**, exporta e baixa o JPG/PNG final —
sem PNGs estáticos, direto da sua conta Canva.

---

## ⚠️ Leia primeiro: requisito de plano

A **Autofill API** (a que substitui placeholders em um Brand Template) tem esta
regra oficial da Canva:

> *"...your integration must act on behalf of a user who is a member of a **Canva
> Enterprise** organization. Users on paid plans have limited trial access
> **during development**."*

O que isso significa para você (assinante **Canva Pro**):
- Criar **Brand Templates** com campos variáveis e ler o dataset → **funciona no Pro** ✅
- Rodar o **Autofill** com sua integração **em modo desenvolvimento** (não
  publicada), **na sua própria conta** → **funciona no Pro** ✅ — que é
  exatamente este caso (um app pessoal de admin, nunca publicado).
- Publicar/distribuir a integração para terceiros → aí a Canva exige **Enterprise**.

Como você vai usar a integração só na sua conta, **mantenha o app em
"Development" no portal** e não submeta para revisão. É assim que fica dentro do
uso permitido no Pro.

---

## 1. Setup no Canva (credenciais + preparar o template)

### 1.1 Criar a integração (Client ID / Client Secret)
1. Acesse o **Portal do Desenvolvedor**: https://www.canva.com/developers/
2. Crie uma **Integration** do tipo **"Public"** ou **"Private"** (para uso
   próprio, "Private/Team" é o ideal — não precisa de revisão).
3. Em **Configuration**, anote o **Client ID** e gere o **Client Secret**
   (guarde o secret com segurança — ele só aparece uma vez).
4. Em **Scopes**, marque exatamente estes:
   - `asset:write`
   - `brandtemplate:meta:read`
   - `brandtemplate:content:read`
   - `design:content:write`
   - `design:content:read`
5. Em **Return URL / Redirect URIs**, adicione **exatamente**:
   ```
   http://127.0.0.1:8080/callback
   ```
   (É o endereço que o script local usa para receber a autorização.)

### 1.2 Preparar o template no Canva (o passo mais importante)
A API só substitui o que estiver marcado como **campo de autofill (Data field)**
em um **Brand Template**. Para cada arte (Domingo, Santa Ceia, etc.):

1. Abra o design no Canva e monte a arte normalmente.
2. **Marque as camadas variáveis como campos:**
   - Selecione a **caixa de texto** do nome → menu (⋯) → **"Add to Brand
     Template data" / "Marcar como campo"** e dê um nome, ex.: `nome`.
   - Repita para a data (`data_evento`) e o horário (`horario`).
   - Selecione o **quadro/frame da foto** (precisa ser um *Frame*, não uma
     imagem solta) → marque como campo, ex.: `foto_pessoa`.
   > Dica: no Canva, a opção aparece como **"Brand Template" data fields**. Se
   > você não vê essa opção, confirme que o design foi **publicado como Brand
   > Template** (botão *Publish → Brand template*).
3. **Publique como Brand Template.**
4. Pegue o **Brand Template ID** na URL do template. Ex.:
   `https://www.canva.com/design/DAFxxxxxxxx/...` → o ID é **`DAFxxxxxxxx`**.

> **Frame para a foto:** para a foto recortar/preencher sozinha sem distorcer,
> o campo de imagem precisa ser um **Frame** do Canva. O Canva ajusta a imagem
> ao frame automaticamente (equivale ao "cover"), preservando o layout.

### 1.3 Descobrir os nomes exatos dos campos
Depois de autorizar (passo 3), rode:
```cmd
python inspecionar_template.py DAFxxxxxxxx
```
Ele lista os campos reais do template (nome e tipo). Use esses nomes na coluna
da direita do `templates.json`.

---

## 2. Mapeamento de Templates (como o código diferencia Domingo × Santa Ceia)

Toda a diferenciação vive no arquivo **`templates.json`** — **sem tocar no
código**. Cada opção do menu aponta para um **Brand Template ID** diferente e diz
quais são os nomes dos campos daquele template:

```json
{
  "id": "santa_ceia",
  "rotulo": "Santa Ceia",
  "brand_template_id": "DAFxxxxxxxx",
  "campos": {
    "foto":    "foto_pessoa",
    "nome":    "nome",
    "data":    "data_evento",
    "horario": "horario"
  }
}
```

- **Esquerda de `campos`** (`foto`, `nome`, `data`, `horario`): nomes internos do
  programa — **não mude**.
- **Direita**: o **nome exato do campo no seu template do Canva** (o que aparece
  no `inspecionar_template.py`). Se em um template você chamou o campo de
  `nome_convidado`, é isso que vai à direita.

Assim, "Domingo" e "Santa Ceia" são apenas duas entradas com **IDs e nomes de
campo próprios**. Para adicionar um novo evento, copie um bloco, troque `id`,
`rotulo`, `brand_template_id` e os `campos`. O menu se atualiza sozinho.

Já deixei **10 opções** prontas (Segunda a Domingo + Jovens, Santa Ceia,
Liderança). Basta colar os IDs.

---

## 3. Instalação e execução

### 3.1 Preparar o ambiente (uma vez)
1. Instale o **Python 3.10+** (marque **"Add Python to PATH"**).
2. Copie **`.env.example`** para **`.env`** e preencha:
   ```
   CANVA_CLIENT_ID=seu_client_id
   CANVA_CLIENT_SECRET=seu_client_secret
   ```
3. Duplo clique em **`instalar.bat`** (instala a biblioteca `requests`).

### 3.2 Autorizar (uma vez)
Duplo clique em **`autorizar.bat`** (ou `python autorizar.py`).
- Abre o navegador → faça login no Canva → **Permitir**.
- O token é salvo em `.canva_token.json`. **Você não repete isso no dia a dia** —
  o programa renova o acesso sozinho (o token de acesso dura 4h e é renovado
  automaticamente pelo refresh token).

### 3.3 Testar a conexão (recomendado antes do 1º uso)
Duplo clique em **`testar.bat`** (ou `python testar_conexao.py`). Ele:
- confere as credenciais e o token (renovando se preciso);
- **conecta de verdade** ao Canva e lista os Brand Templates da sua conta;
- valida cada `brand_template_id` do `templates.json` e checa se os nomes de
  campo que você mapeou realmente existem no template.

Se tudo aparecer com `[OK]`, a geração vai funcionar. É o teste de ponta a ponta
que só pode rodar com as suas credenciais.

### 3.4 Uso diário
Duplo clique em **`gerar_banner.bat`**. O programa:
1. Mostra o **menu de eventos/dias**.
2. Pede **foto** (valida se o arquivo existe), **nome**, **data** e **horário**.
3. Faz upload → autofill → exportação → download.
4. Salva em **`saida/`** (ex.: `santa_ceia_ana_20260721_143005.jpg`).

**Modo por comando (sem perguntas):**
```cmd
python gerar_banner.py --evento santa_ceia --nome "Ana" --data "25/12" --horario "19h30" --foto fotos/ana.jpg
```

---

## 4. Tratamento de erros (o que o programa já resolve sozinho)

O cliente da API (`canva_client.py`) é à prova de falhas:

| Situação | O que o programa faz |
|---|---|
| **Token de acesso expirado** (4h) | Renova automaticamente com o refresh token antes de cada chamada; se pegar um `401` no meio, renova e repete a requisição uma vez. |
| **Refresh token é de uso único** | A cada renovação, salva imediatamente o **novo** refresh token em `.canva_token.json`, para nunca perder o acesso. |
| **Refresh falhou / acesso revogado** | Mostra mensagem clara pedindo para rodar `autorizar.py` de novo. |
| **Rede instável / caiu** | Repete a chamada até 4 vezes com espera crescente (backoff: 1s, 2s, 4s, 8s). |
| **Limite de requisições (`429`)** | Respeita o cabeçalho `Retry-After` e tenta de novo. |
| **Erro de servidor do Canva (`5xx`)** | Backoff e retry automáticos. |
| **Job assíncrono (upload/autofill/export)** | Fica consultando o status até `success`/`failed`, com timeout de 120s; se falhar, mostra o motivo retornado pela API. |
| **Foto inexistente** | Valida **antes** de qualquer chamada e pede o caminho de novo. |
| **Template sem ID configurado** | Avisa qual opção precisa do `brand_template_id` no `templates.json`. |
| **Credenciais ausentes** | Explica como preencher o `.env`. |

Todas as mensagens são em português e sem "telas de erro" técnicas.

---

## 5. Estrutura dos arquivos

```
canva-stories/
├── gerar_banner.py         # CLI principal (menu + inputs + orquestracao)
├── canva_client.py         # Cliente da Canva API (OAuth, upload, autofill, export)
├── autorizar.py            # Autorizacao OAuth PKCE (rode 1 vez)
├── inspecionar_template.py # Descobre os nomes dos campos de um template
├── testar_conexao.py       # Diagnostico: valida credenciais/token/templates
├── templates.json          # Mapeia cada evento -> template_id + campos
├── .env.example            # Modelo das credenciais (copie para .env)
├── requirements.txt        # Dependencia: requests
├── instalar.bat            # Duplo clique: instala dependencias
├── autorizar.bat           # Duplo clique: autoriza no Canva (1 vez)
├── testar.bat              # Duplo clique: testa a conexao / diagnostico
├── gerar_banner.bat        # Duplo clique: gera banners (dia a dia)
├── fotos/                  # Fotos das pessoas
└── saida/                  # Banners finais baixados
```

---

## 6. Fluxo técnico (resumo das chamadas à API)

```
gerar_banner.py
   │
   ├─ POST /v1/asset-uploads           (envia a foto)  ──► poll GET /v1/asset-uploads/{id} ──► asset_id
   │
   ├─ POST /v1/autofills               (brand_template_id + data:{nome,data,horario,foto})
   │                                    ──► poll GET /v1/autofills/{id} ──► design_id
   │
   ├─ POST /v1/exports                 (design_id + format jpg/png 1080x1920)
   │                                    ──► poll GET /v1/exports/{id} ──► urls[]
   │
   └─ GET  {url}                       (baixa o arquivo para saida/)

OAuth (autorizar.py, 1x):
   GET  https://www.canva.com/api/oauth/authorize   (code + PKCE S256)
   POST https://api.canva.com/rest/v1/oauth/token   (troca code -> access+refresh)
```

---

## 7. Problemas comuns

**"Sem autorizacao valida / rode autorizar.py"** — o token não existe, expirou
sem uso por muito tempo, ou o acesso foi revogado. Rode `autorizar.py` de novo.

**"template ... nao tem brand_template_id valido"** — falta colar o ID no
`templates.json` (ou o ID está com o texto `COLE_AQUI...`).

**Autofill retorna erro de campo** — os nomes em `campos` (direita) não batem com
os do template. Rode `inspecionar_template.py <id>` e copie os nomes exatos.

**A foto não preenche direito** — no Canva, o campo de imagem precisa ser um
**Frame**. Imagem solta marcada como campo pode não recortar como esperado.

**"limited trial access" / erro de permissão no autofill** — confirme que o app
está em **Development** e que você autorizou **a mesma conta** dona dos
templates (Canva Pro). Autofill em produção para terceiros exige Enterprise.
