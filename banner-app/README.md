# Gerador de Banners para Stories — App Desktop (Igreja)

Aplicativo de **janela** (dashboard) que cria banners para os Stories do
Instagram, com **renderização local** (grátis, offline). Você desenha as artes
de fundo no **Canva** (Pro), exporta em PNG e o app monta foto + nome(s) + data +
horário por cima, mantendo o layout.

## O que o app faz
- 🖼️ **Dashboard visual** com pré-visualização (não é linha de comando).
- 👥 **Vários participantes** por banner (várias fotos + vários nomes).
- 🤖 **Recomendação automática de foto**: envie várias e o app sugere a melhor
  (analisa nitidez, enquadramento, resolução e iluminação).
- ➕➖ **Gerenciar templates** pela tela: adicionar (importando o PNG do Canva) e
  excluir — sem mexer em código.
- 💾 Salva em **JPG/PNG** 1080×1920 na pasta `saida/`.
- 📦 Vira **.exe** (duplo clique, sem instalar Python) — veja a seção 4.

---

## 1. Instalação (rápida)
1. Instale o **Python 3.10+** (marque **"Add Python to PATH"** no instalador).
2. Duplo clique em **`instalar.bat`** (instala Pillow e numpy).
3. Duplo clique em **`iniciar.bat`** para abrir o app.

> Já vem com **2 templates de exemplo** e **fundos placeholders**, então o app
> abre e gera banner de primeira. Depois você troca pelos seus.

---

## 2. Preparar seus templates (feito no Canva, uma vez por arte)
Aqui o Canva é só a ferramenta de **design** — nada de API.

1. No Canva, monte a arte do Story em **1080×1920**.
2. **Deixe vazios** os espaços onde entram a foto e os textos (nome/data/horário).
3. **Exporte em PNG** (menu *Compartilhar → Baixar → PNG*).
4. Salve o PNG na pasta **`templates_bg/`** (ex.: `domingo.png`).
5. No app, clique em **"Gerenciar templates" → "Importar fundo PNG e criar"**,
   dê um nome e diga quantos participantes tem. Pronto: aparece no menu.

### Ajuste fino de posição
O "Gerenciar templates" cria as posições num padrão inicial. Para mover a foto ou
os textos com precisão, abra **`config/templates.json`** no Bloco de Notas e
ajuste os números (`x`, `y`, `largura`, `altura`, `tamanho`, `cor`...). As
coordenadas são em pixels, a partir do canto superior esquerdo (canvas 1080×1920).

Estrutura de um template:
```json
{
  "id": "domingo",
  "rotulo": "Culto de Domingo",
  "fundo": "templates_bg/domingo.png",
  "fotos": [
    { "x": 290, "y": 560, "largura": 500, "altura": 500, "formato": "circulo" }
  ],
  "textos": [
    { "origem": "nome",    "x": 540, "y": 1110, "tamanho": 78, "cor": "#FFFFFF", "maiusculas": true },
    { "origem": "data",    "x": 540, "y": 1600, "tamanho": 46, "cor": "#FFD700" },
    { "origem": "horario", "x": 540, "y": 1670, "tamanho": 46, "cor": "#FFD700" }
  ]
}
```
- `formato` da foto: `"circulo"`, `"arredondado"` (com `"raio_borda"`) ou `"retangulo"`.
- `origem` do texto: `"nome"`, `"data"`, `"horario"` ou `"fixo"` (com `"texto"`).
- **Vários participantes**: coloque N itens em `fotos` e N textos com
  `"origem": "nome"` (na ordem, o 1º nome vai na 1ª foto).

---

## 3. Uso no dia a dia
1. Abra o app (`iniciar.bat` ou o `.exe`).
2. Escolha o **template** no menu.
3. Para cada pessoa: **"Escolher foto"** (ou **"Recomendar (IA)"** para enviar
   várias e deixar o app pegar a melhor) e digite o **nome**.
4. Preencha **Data** e **Horário**.
5. **"Gerar preview"** para conferir → **"Salvar banner"**.
6. O arquivo vai para **`saida/`**, pronto para postar.

---

## 4. Gerar o executável (.exe)
Para rodar sem precisar de Python instalado (ideal para o dia a dia):

1. Duplo clique em **`gerar_exe.bat`** (instala o PyInstaller e empacota).
2. O executável sai em **`dist\GeradorDeBanners.exe`**.
3. **Importante:** copie estas pastas para **junto do .exe** (mesma pasta):
   `config\`, `templates_bg\`, `fontes\`, `fotos\`, `saida\`.
   (Elas ficam de fora do .exe de propósito, para você adicionar templates e
   fundos sem precisar gerar o .exe de novo.)

Sugestão: crie uma pasta `GeradorDeBanners\`, coloque o `.exe` e essas pastas
dentro, e crie um atalho do `.exe` na área de trabalho.

---

## 5. Recomendação de foto (como a "IA" decide)
Não é um serviço externo nem envia suas fotos para lugar nenhum — roda **offline**.
Para cada foto candidata, calcula uma nota (0–100) combinando:
- **Nitidez** (foco) — penaliza fotos tremidas.
- **Enquadramento** — proporção parecida com o espaço do template (menos corte).
- **Resolução** — pixels suficientes para o tamanho do espaço.
- **Iluminação** — evita fotos muito escuras ou estouradas.

Opcional: se você instalar `opencv-python` (linha comentada no `requirements.txt`),
o app dá um bônus para fotos com **rosto** detectado.

---

## 6. Estrutura dos arquivos
```
banner-app/
├── app.py               # o dashboard (janela)
├── engine.py            # motor de imagem (fundo + fotos + textos)
├── recommender.py       # recomendacao automatica de foto
├── templates_store.py   # ler/gravar templates (adicionar/excluir)
├── config/
│   └── templates.json   # definicao dos templates (posicoes, cores...)
├── templates_bg/        # fundos PNG (exportados do Canva)
├── fontes/              # Montserrat-Bold.ttf (opcional)
├── fotos/               # fotos das pessoas
├── saida/               # banners gerados
├── requirements.txt     # Pillow, numpy
├── instalar.bat         # instala dependencias
├── iniciar.bat          # abre o app
└── gerar_exe.bat        # empacota em .exe
```

---

## 7. Perguntas comuns
**"O fundo do template nao foi encontrado"** — falta o PNG em `templates_bg/`
com o nome que está no `config/templates.json` (campo `fundo`).

**A foto ficou muito cortada** — aumente `largura`/`altura` da foto no template,
ou troque `formato` para `retangulo`.

**O texto está fora do lugar** — ajuste `x`/`y`/`tamanho` daquele texto no
`config/templates.json` e gere o preview de novo.

**Quero um texto fixo** (ex.: "Feliz aniversário,") — adicione um texto com
`"origem": "fixo"` e `"texto": "Feliz aniversario,"`.

**A fonte não é a que eu quero** — coloque o `.ttf` em `fontes/` e aponte em
`config_geral.fonte_padrao` (ou no `fonte` de cada texto).
