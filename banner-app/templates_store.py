#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
================================================================================
  GERENCIADOR DE TEMPLATES (config/templates.json)
================================================================================
Le e grava o arquivo de templates, permitindo ADICIONAR e EXCLUIR templates
pelo dashboard, sem mexer no codigo.
================================================================================
"""

import json
import os
import shutil


class TemplatesStore:
    def __init__(self, base_dir):
        self.base_dir = base_dir
        self.caminho = os.path.join(base_dir, "config", "templates.json")
        self.dados = self._carregar()

    def _carregar(self):
        if not os.path.isfile(self.caminho):
            # Estrutura minima padrao se o arquivo nao existir.
            return {
                "config_geral": {"largura": 1080, "altura": 1920,
                                  "cor_fundo": "#111111",
                                  "fonte_padrao": "fontes/Montserrat-Bold.ttf"},
                "templates": [],
            }
        with open(self.caminho, "r", encoding="utf-8") as f:
            return json.load(f)

    def salvar(self):
        os.makedirs(os.path.dirname(self.caminho), exist_ok=True)
        with open(self.caminho, "w", encoding="utf-8") as f:
            json.dump(self.dados, f, ensure_ascii=False, indent=2)

    # ------------------------------------------------------------------ #
    def config_geral(self):
        return self.dados.get("config_geral", {})

    def listar(self):
        return self.dados.get("templates", [])

    def por_id(self, tid):
        for t in self.listar():
            if t.get("id") == tid:
                return t
        return None

    def por_rotulo(self, rotulo):
        for t in self.listar():
            if t.get("rotulo") == rotulo:
                return t
        return None

    # ------------------------------------------------------------------ #
    def adicionar(self, template):
        """Adiciona (ou substitui, se o id ja existir) um template."""
        existentes = self.dados.setdefault("templates", [])
        for i, t in enumerate(existentes):
            if t.get("id") == template.get("id"):
                existentes[i] = template
                self.salvar()
                return
        existentes.append(template)
        self.salvar()

    def excluir(self, tid):
        """Remove um template pelo id. Devolve True se removeu."""
        existentes = self.dados.get("templates", [])
        novos = [t for t in existentes if t.get("id") != tid]
        removeu = len(novos) != len(existentes)
        self.dados["templates"] = novos
        if removeu:
            self.salvar()
        return removeu

    def importar_fundo(self, caminho_origem, template_id):
        """Copia um PNG de fundo para a pasta templates_bg/ e devolve o caminho
        relativo para salvar no template."""
        destino_dir = os.path.join(self.base_dir, "templates_bg")
        os.makedirs(destino_dir, exist_ok=True)
        ext = os.path.splitext(caminho_origem)[1] or ".png"
        nome = f"{template_id}{ext}"
        destino = os.path.join(destino_dir, nome)
        shutil.copyfile(caminho_origem, destino)
        return os.path.join("templates_bg", nome)


def novo_template(tid, rotulo, fundo_rel, num_fotos=1, largura=1080, altura=1920):
    """Cria um template padrao com N espacos de foto empilhados e campos de
    nome/data/horario em posicoes iniciais razoaveis (voce ajusta depois)."""
    fotos, textos = [], []
    tam_foto = 500 if num_fotos == 1 else 340
    espaco = altura // (num_fotos + 1)

    for i in range(num_fotos):
        cy = espaco * (i + 1) - tam_foto // 2
        fotos.append({
            "x": (largura - tam_foto) // 2, "y": max(120, cy),
            "largura": tam_foto, "altura": tam_foto,
            "formato": "circulo", "raio_borda": 0,
        })
        textos.append({
            "origem": "nome", "x": largura // 2, "y": max(120, cy) + tam_foto + 15,
            "tamanho": 70 if num_fotos == 1 else 48, "cor": "#FFFFFF",
            "ancora": "centro", "maiusculas": True, "sombra": True,
            "largura_max": largura - 120, "prefixo": "",
        })

    base_y = altura - 320
    textos.append({"origem": "data", "x": largura // 2, "y": base_y,
                   "tamanho": 46, "cor": "#FFD700", "ancora": "centro",
                   "maiusculas": False, "sombra": True, "prefixo": ""})
    textos.append({"origem": "horario", "x": largura // 2, "y": base_y + 70,
                   "tamanho": 46, "cor": "#FFD700", "ancora": "centro",
                   "maiusculas": False, "sombra": True, "prefixo": ""})

    return {"id": tid, "rotulo": rotulo, "fundo": fundo_rel,
            "fotos": fotos, "textos": textos}
