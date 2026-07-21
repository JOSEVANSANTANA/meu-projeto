#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  CLIENTE DA CANVA CONNECT API
================================================================================
Encapsula toda a comunicacao HTTP com a API oficial do Canva:
  - Gestao de token OAuth 2.0 (carrega, valida e RENOVA automaticamente).
  - Upload de imagem (asset).
  - Autofill: preenche um Brand Template com Nome/Data/Horario/Foto.
  - Exportacao do design gerado (JPG/PNG).
  - Download do arquivo final.

Este arquivo NAO e executado direto pelo usuario: ele e usado por
  autorizar.py  (para a autorizacao inicial)
  gerar_banner.py (para gerar os banners no dia a dia)

Documentacao oficial: https://www.canva.dev/docs/connect/
================================================================================
"""

import base64
import json
import os
import time

try:
    import requests
except ImportError:
    raise SystemExit(
        "\n[ERRO] A biblioteca 'requests' nao esta instalada.\n"
        "       Rode:  pip install -r requirements.txt\n"
    )

# --------------------------------------------------------------------------- #
# Endpoints oficiais da Canva Connect API
# --------------------------------------------------------------------------- #
AUTORIZAR_URL = "https://www.canva.com/api/oauth/authorize"
TOKEN_URL = "https://api.canva.com/rest/v1/oauth/token"
API_BASE = "https://api.canva.com/rest/v1"

# Escopos (permissoes) que o app precisa. Devem bater com o que voce marcar
# no Portal do Desenvolvedor do Canva.
ESCOPOS = [
    "asset:write",              # enviar a foto
    "brand_template:meta:read", # listar/identificar o template
    "brand_template:content:read",  # ler os campos (dataset) do template
    "design:content:write",     # criar o design via autofill
    "design:content:read",      # exportar o design
]

# Tempo maximo (segundos) esperando um job assincrono (upload/autofill/export).
TIMEOUT_JOB = 120
# Intervalo entre cada checagem de status do job.
INTERVALO_POLL = 2


class CanvaError(Exception):
    """Erro de negocio/API do Canva, com mensagem amigavel em portugues."""


class CanvaClient:
    """Cliente autenticado para a Canva Connect API."""

    def __init__(self, client_id, client_secret, token_store_path):
        self.client_id = client_id
        self.client_secret = client_secret
        self.token_store_path = token_store_path
        self._token = self._carregar_token()

    # ---------------------------------------------------------------------- #
    # TOKEN: carregar, salvar, renovar
    # ---------------------------------------------------------------------- #
    def _carregar_token(self):
        if not os.path.isfile(self.token_store_path):
            return None
        try:
            with open(self.token_store_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, OSError):
            return None

    def _salvar_token(self, dados):
        """Salva o token em disco. IMPORTANTE: o refresh_token e de uso unico,
        entao gravamos o novo imediatamente para nunca perder o acesso."""
        expira_em = dados.get("expires_in", 14400)  # padrao 4h
        registro = {
            "access_token": dados["access_token"],
            "refresh_token": dados["refresh_token"],
            # guardamos o instante de expiracao com 2 min de folga
            "expira_em": int(time.time()) + int(expira_em) - 120,
        }
        with open(self.token_store_path, "w", encoding="utf-8") as f:
            json.dump(registro, f)
        self._token = registro

    def _cabecalho_basic(self):
        """Autenticacao do app: Basic base64(client_id:client_secret)."""
        cred = f"{self.client_id}:{self.client_secret}".encode("utf-8")
        return "Basic " + base64.b64encode(cred).decode("utf-8")

    def trocar_codigo_por_token(self, code, code_verifier, redirect_uri):
        """Passo final do OAuth: troca o 'code' recebido por tokens de acesso.
        Usado apenas uma vez, pelo autorizar.py."""
        resp = requests.post(
            TOKEN_URL,
            headers={
                "Authorization": self._cabecalho_basic(),
                "Content-Type": "application/x-www-form-urlencoded",
            },
            data={
                "grant_type": "authorization_code",
                "code": code,
                "code_verifier": code_verifier,
                "redirect_uri": redirect_uri,
            },
            timeout=30,
        )
        if resp.status_code != 200:
            raise CanvaError(f"Falha ao gerar token: {resp.status_code} - {resp.text}")
        self._salvar_token(resp.json())

    def _renovar_token(self):
        """Usa o refresh_token (uso unico) para obter um novo access_token."""
        if not self._token or not self._token.get("refresh_token"):
            raise CanvaError(
                "Sem autorizacao valida. Rode primeiro:  python autorizar.py"
            )
        resp = requests.post(
            TOKEN_URL,
            headers={
                "Authorization": self._cabecalho_basic(),
                "Content-Type": "application/x-www-form-urlencoded",
            },
            data={
                "grant_type": "refresh_token",
                "refresh_token": self._token["refresh_token"],
            },
            timeout=30,
        )
        if resp.status_code != 200:
            raise CanvaError(
                "Nao foi possivel renovar o acesso ao Canva "
                f"({resp.status_code}). Rode de novo:  python autorizar.py"
            )
        self._salvar_token(resp.json())

    def _access_token(self):
        """Devolve um access_token valido, renovando se estiver expirado."""
        if not self._token:
            raise CanvaError(
                "Voce ainda nao autorizou o app. Rode:  python autorizar.py"
            )
        if time.time() >= self._token.get("expira_em", 0):
            self._renovar_token()
        return self._token["access_token"]

    # ---------------------------------------------------------------------- #
    # REQUISICAO GENERICA (com retry e renovacao de token em 401)
    # ---------------------------------------------------------------------- #
    def _request(self, metodo, caminho, *, tentar_renovar=True, **kwargs):
        """Faz uma chamada a API tratando rede instavel, 429 e token expirado."""
        url = caminho if caminho.startswith("http") else f"{API_BASE}{caminho}"
        headers = kwargs.pop("headers", {})
        headers["Authorization"] = f"Bearer {self._access_token()}"

        ultimo_erro = None
        for tentativa in range(4):  # ate 4 tentativas com backoff exponencial
            try:
                resp = requests.request(metodo, url, headers=headers, timeout=60, **kwargs)
            except requests.RequestException as e:
                ultimo_erro = e
                time.sleep(2 ** tentativa)  # 1s, 2s, 4s, 8s
                continue

            # Token expirou no meio do caminho: renova uma vez e repete.
            if resp.status_code == 401 and tentar_renovar:
                self._renovar_token()
                headers["Authorization"] = f"Bearer {self._token['access_token']}"
                tentar_renovar = False
                continue

            # Limite de requisicoes: espera o tempo sugerido e tenta de novo.
            if resp.status_code == 429:
                espera = int(resp.headers.get("Retry-After", 2 ** tentativa))
                time.sleep(espera)
                continue

            # Erros de servidor (5xx): backoff e retry.
            if 500 <= resp.status_code < 600:
                time.sleep(2 ** tentativa)
                continue

            return resp

        raise CanvaError(f"Falha de conexao com o Canva apos varias tentativas: {ultimo_erro}")

    def _poll_job(self, caminho_job, descricao):
        """Fica checando um job assincrono ate ele terminar (success/failed)."""
        inicio = time.time()
        while time.time() - inicio < TIMEOUT_JOB:
            resp = self._request("GET", caminho_job)
            if resp.status_code != 200:
                raise CanvaError(f"Erro ao consultar {descricao}: {resp.status_code} - {resp.text}")
            job = resp.json().get("job", {})
            status = job.get("status")
            if status == "success":
                return job
            if status == "failed":
                erro = job.get("error", {})
                raise CanvaError(
                    f"{descricao} falhou no Canva: {erro.get('message', erro or 'motivo desconhecido')}"
                )
            time.sleep(INTERVALO_POLL)
        raise CanvaError(f"{descricao} demorou demais (timeout de {TIMEOUT_JOB}s).")

    # ---------------------------------------------------------------------- #
    # 1) UPLOAD DA FOTO (ASSET)
    # ---------------------------------------------------------------------- #
    def upload_asset(self, caminho_foto):
        """Envia a foto local e devolve o asset_id para usar no autofill."""
        if not os.path.isfile(caminho_foto):
            raise CanvaError(f"Foto nao encontrada: {caminho_foto}")

        with open(caminho_foto, "rb") as f:
            binario = f.read()

        nome = os.path.basename(caminho_foto)
        nome_b64 = base64.b64encode(nome.encode("utf-8")).decode("utf-8")

        resp = self._request(
            "POST",
            "/asset-uploads",
            headers={
                "Content-Type": "application/octet-stream",
                "Asset-Upload-Metadata": json.dumps({"name_base64": nome_b64}),
            },
            data=binario,
        )
        if resp.status_code not in (200, 202):
            raise CanvaError(f"Falha ao enviar a foto: {resp.status_code} - {resp.text}")

        job_id = resp.json()["job"]["id"]
        job = self._poll_job(f"/asset-uploads/{job_id}", "Upload da foto")
        return job["asset"]["id"]

    # ---------------------------------------------------------------------- #
    # 2) DATASET DO TEMPLATE (nomes dos campos variaveis)
    # ---------------------------------------------------------------------- #
    def get_dataset(self, brand_template_id):
        """Le os campos (placeholders) de um Brand Template. Util para
        descobrir os nomes exatos a usar no templates.json."""
        resp = self._request("GET", f"/brand-templates/{brand_template_id}/dataset")
        if resp.status_code != 200:
            raise CanvaError(
                f"Nao consegui ler o template {brand_template_id}: "
                f"{resp.status_code} - {resp.text}"
            )
        return resp.json().get("dataset", {})

    # ---------------------------------------------------------------------- #
    # 3) AUTOFILL (preenche o template e cria um novo design)
    # ---------------------------------------------------------------------- #
    def autofill(self, brand_template_id, data, titulo=None):
        """Cria um design preenchendo o template com os dados. Devolve design_id.
        'data' e um dict no formato exigido pela API, por exemplo:
            {
              "nome":    {"type": "text",  "text": "Joao Silva"},
              "foto":    {"type": "image", "asset_id": "Msd..."},
              "data":    {"type": "text",  "text": "25/12"},
              "horario": {"type": "text",  "text": "19h30"}
            }
        """
        corpo = {
            "brand_template_id": brand_template_id,
            "data": data,
        }
        if titulo:
            corpo["title"] = titulo

        resp = self._request(
            "POST", "/autofills",
            headers={"Content-Type": "application/json"},
            data=json.dumps(corpo),
        )
        if resp.status_code not in (200, 202):
            raise CanvaError(f"Falha no autofill: {resp.status_code} - {resp.text}")

        job_id = resp.json()["job"]["id"]
        job = self._poll_job(f"/autofills/{job_id}", "Preenchimento do template")
        return job["result"]["design"]["id"]

    # ---------------------------------------------------------------------- #
    # 4) EXPORTAR o design em JPG/PNG
    # ---------------------------------------------------------------------- #
    def exportar(self, design_id, formato="jpg", largura=1080, altura=1920, qualidade=90):
        """Exporta o design e devolve a lista de URLs de download."""
        formato = formato.lower()
        cfg = {"type": formato, "width": largura, "height": altura}
        if formato in ("jpg", "jpeg"):
            cfg["type"] = "jpg"
            cfg["quality"] = int(qualidade)
        elif formato == "png":
            cfg["export_quality"] = "regular"

        resp = self._request(
            "POST", "/exports",
            headers={"Content-Type": "application/json"},
            data=json.dumps({"design_id": design_id, "format": cfg}),
        )
        if resp.status_code not in (200, 202):
            raise CanvaError(f"Falha ao exportar: {resp.status_code} - {resp.text}")

        job_id = resp.json()["job"]["id"]
        job = self._poll_job(f"/exports/{job_id}", "Exportacao")
        urls = job.get("urls", [])
        if not urls:
            raise CanvaError("A exportacao terminou mas nao retornou nenhum arquivo.")
        return urls

    # ---------------------------------------------------------------------- #
    # 5) BAIXAR o arquivo exportado
    # ---------------------------------------------------------------------- #
    def baixar(self, url, destino):
        """Baixa a imagem exportada para um caminho local."""
        for tentativa in range(4):
            try:
                resp = requests.get(url, timeout=120, stream=True)
                resp.raise_for_status()
                with open(destino, "wb") as f:
                    for pedaco in resp.iter_content(chunk_size=8192):
                        f.write(pedaco)
                return destino
            except requests.RequestException as e:
                if tentativa == 3:
                    raise CanvaError(f"Falha ao baixar o arquivo final: {e}")
                time.sleep(2 ** tentativa)
