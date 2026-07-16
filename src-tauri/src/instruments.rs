//! Resolução de ativos por CÓDIGO ou NOME.
//!
//! O usuário digita coisas como "mesu6", "es", "micro nasdaq" ou "dow" e
//! precisamos vincular ao instrumento real para a IA analisar corretamente
//! (índices reagem de forma diferente à mesma notícia). Cobre os principais
//! futuros de índice da CME/CBOT negociados (E-mini e Micro E-mini).

pub struct Instrument {
    /// Raiz canônica do símbolo Globex (ex.: "MES").
    pub code: &'static str,
    /// Nome legível exibido e injetado no prompt (ex.: "Micro E-mini S&P 500 (MES)").
    pub name: &'static str,
    /// Tokens (minúsculos) para casar por nome digitado.
    pub aliases: &'static [&'static str],
}

#[derive(serde::Serialize, Clone)]
pub struct Resolved {
    pub code: String,
    pub name: String,
    pub recognized: bool,
}

// Micros ANTES dos minis: assim "micro nasdaq" casa MNQ (não NQ) no passo de
// substring. O casamento por código é exato e independe da ordem.
static INSTRUMENTS: &[Instrument] = &[
    Instrument {
        code: "MES",
        name: "Micro E-mini S&P 500 (MES)",
        aliases: &["mes", "micro s&p", "micro sp", "micro es", "micro s&p 500"],
    },
    Instrument {
        code: "MNQ",
        name: "Micro E-mini Nasdaq-100 (MNQ)",
        aliases: &["mnq", "micro nasdaq", "micro nq", "micro nasdaq-100"],
    },
    Instrument {
        code: "MYM",
        name: "Micro E-mini Dow Jones (MYM)",
        aliases: &["mym", "micro dow", "micro ym", "micro dow jones"],
    },
    Instrument {
        code: "M2K",
        name: "Micro E-mini Russell 2000 (M2K)",
        aliases: &["m2k", "micro russell", "micro russell 2000"],
    },
    Instrument {
        code: "ES",
        name: "E-mini S&P 500 (ES)",
        aliases: &["es", "sp500", "s&p500", "s&p 500", "sp 500", "e-mini s&p 500", "mini sp"],
    },
    Instrument {
        code: "NQ",
        name: "E-mini Nasdaq-100 (NQ)",
        aliases: &["nq", "nasdaq", "nasdaq-100", "nasdaq 100", "ndx"],
    },
    Instrument {
        code: "YM",
        name: "E-mini Dow Jones (YM)",
        aliases: &["ym", "dow", "dow jones", "djia"],
    },
    Instrument {
        code: "RTY",
        name: "E-mini Russell 2000 (RTY)",
        aliases: &["rty", "russell", "russell 2000", "russell2000"],
    },
    Instrument {
        code: "EMD",
        name: "E-mini S&P MidCap 400 (EMD)",
        aliases: &["emd", "midcap", "mid cap", "s&p midcap 400", "midcap 400"],
    },
    Instrument {
        code: "NKD",
        name: "Nikkei 225 (NKD)",
        aliases: &["nkd", "niy", "nikkei", "nikkei 225"],
    },
];

const MONTH_CODES: &[u8] = b"FGHJKMNQUVXZ";

/// De um código de futuro como "MESU6" extrai a raiz "MES" (remove mês+ano).
/// Retorna None se não houver sufixo de ano (dígitos no fim).
fn futures_root(compact_upper: &str) -> Option<String> {
    let bytes = compact_upper.as_bytes();
    let mut end = bytes.len();
    let mut digits = 0;
    while end > 0 && bytes[end - 1].is_ascii_digit() && digits < 2 {
        end -= 1;
        digits += 1;
    }
    if digits == 0 || end == 0 {
        return None; // sem ano -> não tratamos como código de futuro
    }
    let month = bytes[end - 1];
    if !MONTH_CODES.contains(&month.to_ascii_uppercase()) {
        return None;
    }
    let root = &compact_upper[..end - 1];
    if root.len() < 2 {
        return None;
    }
    Some(root.to_string())
}

fn recognized(inst: &Instrument) -> Resolved {
    Resolved {
        code: inst.code.to_string(),
        name: inst.name.to_string(),
        recognized: true,
    }
}

fn by_code(code: &str) -> Option<&'static Instrument> {
    INSTRUMENTS.iter().find(|i| i.code.eq_ignore_ascii_case(code))
}

/// Detecta a família do índice pelo texto e escolhe micro vs mini conforme a
/// presença de "micro". Resolve nomes por extenso como
/// "Micro E-mini Nasdaq-100 futuro" (onde "micro" e "nasdaq" não são contíguos).
/// Ordem importa: midcap antes de s&p; s&p (amplo) por último.
fn resolve_family(norm: &str) -> Option<&'static Instrument> {
    let micro = norm.contains("micro");
    // (palavras-chave do índice, código micro, código mini)
    let families: &[(&[&str], &str, &str)] = &[
        (&["midcap", "mid cap", "s&p midcap", "400"], "EMD", "EMD"),
        (&["nasdaq", "ndx"], "MNQ", "NQ"),
        (&["russell", "2000"], "M2K", "RTY"),
        (&["dow", "djia"], "MYM", "YM"),
        (&["nikkei", "niy"], "NKD", "NKD"),
        (&["s&p", "sp 500", "s&p 500", "500"], "MES", "ES"),
    ];
    for (keys, micro_code, mini_code) in families {
        if keys.iter().any(|k| norm.contains(k)) {
            return by_code(if micro { micro_code } else { mini_code });
        }
    }
    None
}

/// Resolve a entrada do usuário (código ou nome) para um instrumento.
pub fn resolve(input: &str) -> Resolved {
    let raw = input.trim();
    if raw.is_empty() {
        return Resolved {
            code: String::new(),
            name: String::new(),
            recognized: false,
        };
    }
    let norm = raw.to_lowercase();
    let compact_upper: String = norm
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_uppercase();

    // 1. Código: direto ou via raiz de futuro (MESU6 -> MES).
    let mut candidates = vec![compact_upper.clone()];
    if let Some(root) = futures_root(&compact_upper) {
        candidates.push(root);
    }
    for cand in &candidates {
        for inst in INSTRUMENTS {
            if inst.code.eq_ignore_ascii_case(cand) {
                return recognized(inst);
            }
        }
    }

    // 2. Nome — casamento exato de alias.
    for inst in INSTRUMENTS {
        if inst.aliases.iter().any(|a| *a == norm) {
            return recognized(inst);
        }
    }

    // 3. Nome por família + micro/mini (ex.: "Micro E-mini Nasdaq-100 futuro").
    if let Some(inst) = resolve_family(&norm) {
        return recognized(inst);
    }

    // 4. Fallback: substring de alias (micros primeiro pela ordem da lista).
    for inst in INSTRUMENTS {
        if inst.aliases.iter().any(|a| a.len() >= 3 && norm.contains(a)) {
            return recognized(inst);
        }
    }

    // Não reconhecido: usa o texto cru (análise genérica), mas sinaliza.
    Resolved {
        code: raw.to_uppercase(),
        name: raw.to_string(),
        recognized: false,
    }
}
