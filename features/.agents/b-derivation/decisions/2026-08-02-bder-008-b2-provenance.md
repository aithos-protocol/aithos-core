# Décision — BDER-008 : provenance du vecteur `b2-derivation.json`

| Champ | Valeur |
|---|---|
| Finding | `BDER-008` (P3, `OUVERT` — décision listée par l'audit public) |
| Date | 2026-08-02 |
| Décideur | Propriétaire du protocole (Mathieu) |
| Statut résultant | `CORRECTION_REQUESTED` (ronde 2) |

## Décision

**Corriger honnêtement la revendication de provenance, sans changer aucune
valeur du vecteur.** Le générateur indépendant reste la cible d'un lot futur.

1. La `description` de `b2-derivation.json` cesse d'affirmer une génération
   indépendante non reproductible (« generated independently (Python blake3) »).
   Elle énonce la provenance réelle : vecteur, fixtures et test créés dans le
   même commit `1b7d258` ; `folder1_key_hex` corroboré par cinq scripts Python,
   `deep_section_key_hex` par un ; `sibling_section_sid`,
   `sibling_section_key_hex` et `tag` sans témoin externe (auto-certifiés par
   `derive.rs` via `b2_derivation.rs`).
2. La règle du vecteur figé tient : **aucune valeur ne change**.
3. **Porte laissée ouverte** : un `gen-b2-derivation.py` indépendant, nommé
   dans la `description` conformément à `vectors/README.md:8-11`, reste la
   seule voie de fermeture de `BDER-007`. Lot futur, sans échéance imposée par
   ce cycle. Ce lot devrait aussi brancher en CI les garde-fous B2 existants de
   `gen-f/g/h/h2/i` (`ci.yml` ne lance que `fmt`, `clippy`, `test`).

## Motifs

- L'honnêteté des claims prime : une fausse promesse d'indépendance est pire
  qu'une absence d'indépendance assumée et documentée.
- Correction documentaire immédiate, sans risque sur les valeurs ni les tests.
- Un générateur écrit sous pression de clôture, en regardant le code Rust,
  n'apporterait pas l'indépendance recherchée.

## Conséquences exécutables

- Ronde 2 `b-derivation` (correcteur) : réécrire la `description` du vecteur.
- `BDER-007` reste `OUVERT` et visible jusqu'au lot générateur.
