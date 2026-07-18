//! Configurações persistidas em disco (`settings.json` no diretório de dados
//! do app). Guarda as API keys adicionadas pela UI e o ativo prioritário, para
//! que o usuário informe a chave UMA vez e ela sobreviva a reinícios.
//!
//! Observação de segurança: as chaves ficam em texto plano no perfil local do
//! usuário (mesma sensibilidade de um `.env`). É um app local pessoal.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub api_keys: Vec<String>,
    #[serde(default)]
    pub asset: Option<String>,
    /// Feeds RSS adicionados pelo usuário na UI.
    #[serde(default)]
    pub rss_feeds: Vec<String>,
}

/// Lê o settings.json (retorna padrão vazio se não existir/for inválido).
pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Grava o settings.json (falha silenciosa: persistência é best-effort).
pub fn save(path: &Path, settings: &Settings) {
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir();
        let path = dir.join("esf_settings_test.json");
        let s = Settings {
            api_keys: vec!["k1".into(), "k2".into()],
            asset: Some("E-mini Nasdaq-100 (NQ)".into()),
            rss_feeds: vec!["https://x.com/rss".into()],
        };
        save(&path, &s);
        let loaded = load(&path);
        assert_eq!(loaded.api_keys, vec!["k1", "k2"]);
        assert_eq!(loaded.asset.as_deref(), Some("E-mini Nasdaq-100 (NQ)"));
        assert_eq!(loaded.rss_feeds, vec!["https://x.com/rss"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn ausente_retorna_padrao() {
        let s = load(Path::new("/caminho/inexistente/settings.json"));
        assert!(s.api_keys.is_empty());
        assert!(s.asset.is_none());
    }
}
