# Gerador de Banners para Stories do Instagram (Igreja)

Programa de linha de comando (CMD do Windows) que cria banners padronizados para
os Stories, mantendo o layout do template do Canva intacto. Você informa **dia da
semana**, **nome** e **foto** — o programa recorta a foto sem distorcer, escreve o
nome com a fonte/cor/posição do dia e salva a imagem pronta para postar.

---

## 1. Arquitetura da Solução (a recomendação)

Você tem dois caminhos possíveis. Abaixo a comparação honesta:

| Critério | **Canva API (Autocreate)** | **Python + Pillow (esta solução)** ✅ |
|---|---|---|
| Custo | Exige plano pago + **aprovação de app** pela Canva; a API de Autofill/Autocreate é liberada caso a caso e voltada a integrações empresariais | **100% grátis** |
| Dependência de internet | Sim, toda geração depende da nuvem da Canva | **Nenhuma** — roda offline no seu PC |
| Complexidade de setup | Alta: criar app, OAuth, tokens, aprovação | **Baixa**: instalar Python e rodar |
| Controle do layout | Bom, mas preso ao formato da API | **Total** — você controla cada pixel |
| Risco de mudar/quebrar | API pode mudar termos/preços | Estável, sob seu controle |

**Recomendação: Python + Pillow com templates de fundo exportados do Canva.**

Motivo: a API de Autocreate da Canva não é um serviço aberto de autoatendimento —
depende de aprovação e de plano compatível, o que é exagerado para o seu caso. Já
o modelo **"template PNG + sobreposição local"** é gratuito, roda offline, é à prova
de falhas e mantém o Canva onde ele é melhor: **desenhar a arte**. Você desenha os
5 fundos no Canva, exporta em PNG, e o Python só faz o trabalho repetitivo (encaixar
a foto e escrever o nome).

### Como o fluxo funciona

```
   Canva (você desenha 1x)           Python (roda todo dia, automático)
  ┌──────────────────────┐          ┌───────────────────────────────────┐
  │ 5 fundos PNG          │  --->    │ 1. Abre o template do dia          │
  │ (seg a sex, sem foto  │          │ 2. Recorta a foto (sem distorcer)  │
  │  e sem nome)          │          │ 3. Cola a foto na posição certa    │
  └──────────────────────┘          │ 4. Escreve o nome (fonte/cor/pos)  │
                                     │ 5. Salva em /saida (pronto p/ post)│
                                     └───────────────────────────────────┘
```

---

## 2. Configuração do Ambiente (Windows, passo a passo)

### Passo 1 — Instalar o Python
1. Acesse **https://www.python.org/downloads/** e baixe o Python (3.10 ou superior).
2. Rode o instalador e, **MUITO IMPORTANTE**, marque a caixa
   **"Add Python to PATH"** na primeira tela, antes de clicar em *Install Now*.
3. Para conferir: abra o **CMD** (tecla Windows, digite `cmd`, Enter) e digite:
   ```cmd
   python --version
   ```
   Deve aparecer algo como `Python 3.12.x`.

> Você **não precisa** instalar Node.js. Esta solução é só Python.

### Passo 2 — Baixar os arquivos do programa
Copie a pasta `banner-stories` (esta pasta) para um lugar fácil, por exemplo:
`C:\banners\banner-stories`.

### Passo 3 — Instalar as dependências (só uma vez)
Dê **duplo clique** no arquivo **`instalar.bat`**.
Ele instala automaticamente a biblioteca de imagens (Pillow). Espere aparecer
"Pronto!" e feche a janela.

> Prefere fazer manual? Abra o CMD na pasta e rode:
> ```cmd
> pip install -r requirements.txt
> ```

### Passo 4 — Colocar os templates, a fonte e as fotos
- **Templates**: exporte os 5 fundos do Canva em **PNG 1080×1920** (sem foto e sem
  nome) e salve na pasta `templates/` com os nomes: `segunda.png`, `terca.png`,
  `quarta.png`, `quinta.png`, `sexta.png`. (Veja `templates/LEIA-ME.txt`.)
- **Fonte**: baixe a **Montserrat** em
  https://fonts.google.com/specimen/Montserrat, extraia e copie o arquivo
  `Montserrat-Bold.ttf` para a pasta `fontes/`. (Pode trocar por outra fonte no
  `config.json`.)
- **Fotos**: coloque as fotos das pessoas na pasta `fotos/`.

> O programa **funciona mesmo sem** template/fonte (gera com fundo liso e fonte
> padrão, apenas avisando), então você pode testar antes de ter a arte pronta.

---

## 3. Como usar no dia a dia

### Jeito mais fácil (recomendado para uso diário)
Dê **duplo clique** em **`gerar_banner.bat`**. O programa abre e pergunta:
1. **O dia da semana** (digite o nome ou o número 1–5).
2. **O nome da pessoa**.
3. **O caminho da foto** (ex.: `fotos/joao.jpg`).

Pronto: o banner sai na pasta **`saida/`**, com nome tipo
`segunda_joao_da_silva_20260721_143005.jpg`.

### Jeito por comando (rápido, sem perguntas)
Abra o CMD na pasta e rode tudo de uma vez:
```cmd
python gerar_banner.py --dia segunda --nome "João Silva" --foto fotos/joao.jpg
```

Opções:
- `--dia` : `segunda | terca | quarta | quinta | sexta` (ou `1` a `5`)
- `--nome` : nome que aparece no banner (use aspas se tiver espaço)
- `--foto` : caminho da foto
- `--formato` : `JPG` (padrão) ou `PNG`

Se você esquecer algum parâmetro, o programa **pergunta** só o que faltou.

---

## 4. Ajustar o layout (posição da foto e do texto)

Tudo é controlado pelo arquivo **`config.json`** — você **não precisa mexer no
código**. Abra com o Bloco de Notas e altere os números. O canvas é 1080×1920 e as
coordenadas contam a partir do **canto superior esquerdo**.

Exemplo de um dia:
```json
"segunda": {
  "template": "templates/segunda.png",
  "foto": {
    "x": 240, "y": 520,          // canto superior esquerdo da foto
    "largura": 600, "altura": 600,
    "formato": "circulo",         // "circulo", "arredondado" ou "retangulo"
    "raio_borda": 40              // usado quando formato = arredondado
  },
  "texto": {
    "x": 540, "y": 1240,          // posição do nome
    "fonte": "fontes/Montserrat-Bold.ttf",
    "tamanho": 90,
    "cor": "#FFFFFF",             // cor em hexadecimal
    "ancora": "centro",           // "centro", "esquerda" ou "direita"
    "largura_max": 900,           // quebra o nome em 2 linhas se for muito longo
    "maiusculas": true,           // deixa o nome em MAIÚSCULAS
    "sombra": true                // sombra suave para legibilidade
  }
}
```

**Dica para achar as coordenadas certas:** no Canva, veja a posição/tamanho do
espaço da foto e do texto (em px, num design de 1080×1920) e copie os valores para
o `config.json`. Gere um banner de teste e ajuste fino.

---

## 5. Como o programa é robusto (à prova de falhas)

- ✅ **Valida se a foto existe** antes de processar (e pede de novo se estiver errado).
- ✅ **Corrige a rotação** de fotos tiradas no celular (metadados EXIF).
- ✅ **Recorta sem distorcer** (estilo "cover": preenche o espaço mantendo a proporção).
- ✅ **Máscara circular / cantos arredondados** com bordas suaves (antialiasing).
- ✅ **Não quebra** se faltar template ou fonte — gera mesmo assim e avisa.
- ✅ **Nome de arquivo único** (dia + nome + data/hora) — nunca sobrescreve.
- ✅ **Não altera o template**: a arte original é preservada; só somamos foto e texto.
- ✅ Mensagens de erro **em português e amigáveis**, sem "telas de erro" técnicas.

---

## 6. Estrutura de arquivos

```
banner-stories/
├── gerar_banner.py     # o programa (código-fonte comentado)
├── config.json         # layout de cada dia (edite aqui, sem programar)
├── requirements.txt    # dependências (Pillow)
├── instalar.bat        # duplo clique: instala dependências (1ª vez)
├── gerar_banner.bat    # duplo clique: abre o programa (uso diário)
├── templates/          # os 5 fundos PNG do Canva (segunda.png ... sexta.png)
├── fontes/             # a fonte .ttf (Montserrat-Bold.ttf)
├── fotos/              # as fotos das pessoas
└── saida/              # onde os banners prontos são salvos
```

---

## 7. Perguntas comuns

**A foto ficou cortada demais.** Aumente a `largura`/`altura` da foto no
`config.json`, ou ajuste o `x`/`y` para reposicionar.

**Quero a foto quadrada, não redonda.** Troque `"formato": "circulo"` por
`"retangulo"` (ou `"arredondado"` com um `raio_borda`).

**O nome longo está estourando.** Diminua `tamanho`, aumente `largura_max` (para
quebrar em 2 linhas) ou reposicione com `y`.

**Quero um texto fixo antes do nome** (ex.: "Feliz aniversário,"). Preencha
`"conteudo_prefixo": "Feliz aniversario, "` no `config.json` daquele dia.

**Posso usar sábado/domingo?** Sim. Copie um bloco de dia no `config.json`, dê o
nome `sabado`/`domingo`, adicione o alias correspondente e o template.
