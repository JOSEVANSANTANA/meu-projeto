# ESF News Monitor — iOS (Tauri Mobile)

Este projeto foi preparado para compilar como app **iOS nativo** reaproveitando
o mesmo backend Rust + frontend React (Tauri 2 Mobile). Este guia tem os passos
para **gerar, rodar e instalar no seu iPhone**.

> ⚠️ **Verdade técnica importante:** um `.ipa` que instala no seu iPhone só pode
> ser **compilado e assinado no macOS com Xcode**, usando a **sua conta Apple
> Developer** (assinatura amarrada ao seu dispositivo). Não existe atalho por
> Linux/Windows. Os passos abaixo rodam **no seu Mac** (ou num Mac na nuvem).

---

## O que muda no iOS vs. desktop (leia antes)

- **App de PRIMEIRO PLANO.** O iOS suspende apps em segundo plano — não existe o
  loop 24/7 do desktop. Na prática você abre o app e o deixa aberto durante o
  pregão; a coleta roda enquanto ele está na tela. Alertas com o app fechado
  exigiriam um **servidor + push (APNs)**, que é outra arquitetura (posso montar
  depois, se quiser).
- **Conector LOGADO do Truth Social: desativado no iOS.** Ele usa subprocesso
  Python (`curl_cffi`), proibido no sandbox do iOS. O **espelho do Trump (RSS)**
  e todos os demais feeds funcionam normalmente.
- **Auto-update: desativado no iOS.** No iPhone a atualização vem pela App Store
  / TestFlight (ambos já isolados por plataforma no código).
- **Chave Gemini:** cole no campo do app — ela é **persistida** no sandbox do
  app (`settings.json`), então sobrevive a reinícios, igual ao desktop.

---

## Pré-requisitos (no Mac)

1. **macOS + Xcode** (instale pela App Store) e as ferramentas de linha de comando:
   ```bash
   xcode-select --install
   ```
2. **Conta Apple Developer.** Grátis serve para rodar no SEU aparelho (com
   validade de 7 dias por build); a paga (US$99/ano) permite distribuição e
   builds sem expirar.
3. **Rust + alvos iOS:**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
   ```
4. **Node 20+** e **CocoaPods:**
   ```bash
   brew install cocoapods
   ```

---

## Passo a passo

Na raiz do projeto (o mesmo `esfnewsmonitor`):

```bash
npm install

# 1. Gera o projeto Xcode do iOS (uma vez). Cria src-tauri/gen/apple/.
npm run tauri ios init

# 2. Rodar no SIMULADOR (rápido, não precisa de conta paga):
npm run tauri ios dev
#    -> escolha um simulador (ex.: iPhone 15) na lista.

# 3. Rodar no SEU iPhone físico:
#    a) conecte o iPhone via cabo e confie no computador;
#    b) abra o projeto no Xcode:
npm run tauri ios dev --open        # abre o Xcode
#    c) no Xcode: selecione o alvo, aba "Signing & Capabilities",
#       marque "Automatically manage signing" e escolha seu Team (Apple ID);
#       selecione seu iPhone no topo e clique ▶ (Run).
#    d) no iPhone: Ajustes > Geral > Gerenciamento de Dispositivo >
#       confie no seu perfil de desenvolvedor.
```

### Gerar o instalador (.ipa) para distribuir

```bash
npm run tauri ios build            # produz o .ipa assinado
# saída em: src-tauri/gen/apple/build/ ... .ipa
```

Para instalar em outros iPhones sem cabo, use **TestFlight** (App Store Connect)
— é o caminho oficial para beta com clientes.

---

## Notas de engenharia para o iOS (follow-ups)

- **Permissão de notificação:** no iOS o app precisa PEDIR permissão de
  notificação em runtime na primeira vez. Se as notificações nativas não
  aparecerem, adicionamos a chamada de `requestPermission` do plugin de
  notificação no boot do app (posso fazer quando você testar).
- **Layout:** a UI foi desenhada para telas largas (terminal Bloomberg). No
  iPhone ela funciona, mas vale um passe de responsividade (colunas empilhadas)
  para ficar confortável no retrato — fácil de ajustar depois do primeiro run.
- **Cotações/Yahoo e feeds:** funcionam no iOS (é só HTTP). O ATS do iOS exige
  HTTPS — todos os nossos endpoints já são HTTPS. ✔

---

## O que foi verificado aqui (Linux) e o que só dá para verificar no Mac

- ✔ **Verificado:** o código compila para **desktop** com as partes iOS-
  incompatíveis isoladas (subprocesso e auto-update por `cfg`/target), 12/12
  testes passando, sem warnings. Ou seja, a base está pronta para o `tauri ios`.
- ⚠️ **Só no seu Mac:** a compilação iOS em si (`tauri ios init/build`), a
  assinatura e o run no aparelho — porque exigem Xcode + sua conta Apple, que
  não existem neste ambiente Linux. Não dá para eu gerar/assinar o `.ipa` daqui.
