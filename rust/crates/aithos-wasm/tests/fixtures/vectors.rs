//! Résolution des fixtures de vecteurs de conformance (lot SPL-1 du
//! chantier split repo).
//!
//! Coupe la dépendance des tests à la profondeur de répertoire du
//! monorepo : le répertoire des vecteurs vient de `AITHOS_VECTORS_DIR`
//! quand la variable est définie, sinon du chemin relatif historique
//! (`<crate>/../../../vectors`). Aucun vecteur n'est copié ni modifié.
//!
//! Ce fichier est inclus tel quel par chaque crate consommateur
//! (`#[path]` depuis les crates de tests, `include!` depuis les modules
//! `#[cfg(test)]` de `src/`) : `CARGO_MANIFEST_DIR` s'évalue donc dans
//! le crate incluant, ce qui donne le bon repli par crate.

use std::path::PathBuf;

/// Répertoire des vecteurs : `AITHOS_VECTORS_DIR` ou le chemin du monorepo.
pub fn vectors_dir() -> PathBuf {
    match std::env::var_os("AITHOS_VECTORS_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../vectors")),
    }
}

/// Lit un vecteur par nom de fichier (ex. `cb2-session-proof.json`).
pub fn vector_str(name: &str) -> String {
    let path = vectors_dir().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("vector fixture `{}` unreadable: {e}", path.display()))
}
