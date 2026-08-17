# Aithos — Glossary

**Protocol V1 · document v3.6 · 2026-08-17**

Definitions of the terms used by the specification. No rule is born here: when a definition touches a rule, it cites it — and if, in order to be clear, it takes up the rule's terms, the rule alone governs in case of discrepancy.

**Access** *(accès)* — What a current mandate, or ownership of the Ethos, gives to an identity.

**Access graph** *(graphe des accès)* — Who holds which rights over which identifiers (R-10.4).

**Activity graph** *(graphe d'activité)* — Who moved which records, when, and by what type of act (R-10.4, X-21).

**Actor** *(acteur)* — The owner, or a holder.

**Add** *(ajouter)* — Right. Create a file or a folder within a folder.

**Aithos** — The protocol that this document specifies. A vault that follows it is an Ethos.

**Anonymous shape** *(forme anonyme)* — What a subtree shows without its names or its contents: how many nodes, and how they are nested (X-25).

**Artifact** *(artefact)* — The material form that holds an Ethos on its medium: what verification examines (R-8.14).

**Audit** *(auditer)* — Right. Read the journal of a node and of its content: the acts, their authors, the identifiers of the nodes concerned, the mandates invoked, the fingerprints.

**Auditor** *(auditeur)* — Holder of a mandate that includes *audit*.

**Author's mark** *(marque d'auteur)* — Unforgeable attestation borne by a node: its last author, the mandate invoked, and the state written (R-2.7).

**Binding** *(opposable)* — Said of a receipt whose entry the owner cannot discard without also discarding a later entry from an identity other than their own (R-9.4).

**Browse** *(parcourir)* — Right. See the index of a folder and of everything it contains, at any depth.

**Canonical state** *(état canonique)* — The state of the tree and of the mandates produced by the valid entries alone (R-8.15); who computes it is fixed by X-26.

**Chain** *(chaîne)* — The sequence of mandates linking a holder to the owner.

**Close** *(refermer)* — Right. Make a node, its name and all its content private.

**Compatible** *(compatibles)* — Said of two receipts, one of which is the continuation of the other, or which freeze the same instant identically (R-9.5).

**Concern** *(concerner)* — Said of the journal entries that touch a node (R-8.6).

**Conformant tool** *(outil conforme)* — A tool whose acts comply with the rules of Part I.

**Containment** *(contenance)* — The nesting of records, as the activity graph traces it through successive writes — anonymous: without name or content (X-21, R-10.3).

**Continuous observer** *(observateur continu)* — Anyone who observes the Ethos over time, without necessarily holding any access; what that observation yields is fixed by X-21 (X-24, X-29).

**Corrupted** *(corrompu)* — Said of an Ethos whose artifact has been altered outside the protocol: its verification fails (R-8.14).

**Countersignature** *(contreseing)* — Entry by which a holder attests to the state of the journal, without modifying anything else.

**Delegate** *(délégué)* — Holder of a mandate, whoever its issuer. Another name for the holder, used when the chain matters; no normative difference between the two.

**Delegate (right)** *(déléguer)* — Right. Issue a mandate that does not exceed one's own.

**Delete** *(supprimer)* — Right. Delete a file or a folder contained in a folder, or empty a folder.

**Descend** *(descendre)* — Said of a mandate issued under another, directly or through successive sub-delegations: the relation that grounds cascading revocation (R-7.2).

**Designate** *(désigner)* — Indicate a node in order to act upon it, by its identifier.

**Edit** *(éditer)* — Right. Replace the content of a file.

**Empty** *(vider)* — Act. Delete, in a single act, all the content of a folder, without designating its elements (R-5.9).

**Entry** *(entrée)* — Recording of an act in the journal; what it bears is fixed by R-8.3.

**Epoch** *(époque)* — The state of the held form of a subtree between two re-sealings: it changes at re-sealing (R-7.11), and a write from a stale epoch is void (R-8.15).

**Equivocation** *(équivoque)* — Two concurrent continuations of one and the same journal, each presented to different actors (R-1.7).

**Ethos** — A vault: a tree of nodes, a journal, mandates.

**Ethos identity** *(identité de l'Ethos)* — Mark proper to an Ethos, fixed at its creation and derived from the act of creation, distinct from the owner's identity.

**Fingerprint** *(empreinte)* — Short mark of a specific content: whoever holds that content can verify that it corresponds to it. It does not disclose the content, but it allows an exact guess to be confirmed (X-19).

**Formal validity** *(validité formelle)* — What, of a chain, can be verified on the chain alone, by anyone; what it covers — and does not cover — is fixed by R-6.12.

**Former holder** *(ancien titulaire)* — Identity whose mandate has ended.

**Held form** *(forme tenue)* — The form in which the Ethos holds contents, names and indexes on its medium: what every creation extends, and what publishing, closing and re-sealing switch over (R-1.11, R-3.12, R-7.11).

**Holder** *(titulaire)* — Identity holding a current mandate.

**Identifier** *(identifiant)* — Mark proper to a node, stable, opaque, which reveals neither its name, nor its place, nor its content.

**Identity** *(identité)* — A secret and its public mark. It designates a bearer of a secret, not a person.

**Identity secret** *(secret d'identité)* — The part of an identity that does not show itself, and by which it acts.

**Index** — List of the nodes contained in a folder, with their names.

**Invalid** *(invalide)* — Said of an act contrary to the rules — its entry is void (R-8.15) — or of a node whose entry is missing (R-8.13).

**Issuer** *(émetteur)* — Actor who has issued a mandate.

**Journal** — Ordered sequence of entries, append-only.

**Link** *(maillon)* — A mandate, considered as an element of a chain.

**Mandate** *(mandat)* — A scope and a set of rights, granted to an identity.

**Medium** *(support)* — What materially holds an Ethos. Anyone may hold it (R-1.10).

**Methods** *(procédés)* — The means that materially hold an Ethos, named by its identity from creation onward (R-1.13). Their ageing is an assumed limit (X-22).

**Name** *(nom)* — Appellation of a node, borne by the index of its parent folder.

**Node** *(nœud)* — A file or a folder. Unit of visibility and of delegation.

**Order** *(ordre)* — Position of an entry in the journal.

**Owner** *(propriétaire)* — The identity declared at the creation of the Ethos.

**Portion** *(part)* — What, of an entry, concerns a given node; who reads which portion is fixed by R-8.7.

**Private** *(privé)* — Said of a node whose content and name can be learned only by an identity holding, at the moment it learns them, a current access — whatever tool anyone employs, and within the limits that R-7.3 and X-27 assume before re-sealing. What has been learned during an access is deemed learned (R-1.12).

**Proof of continuation** *(preuve de continuation)* — Proof that the present state of the journal is the continuation of the state frozen by a receipt (R-9.6).

**Public** — Said of a node whose content and name are readable by anyone, without any access.

**Public mark** *(marque publique)* — The part of an identity that shows itself. It designates the identity, and allows what its bearer attests to be verified.

**Publication point** *(point de publication)* — The node on which the decision to publish was taken, so long as no higher publication has absorbed that quality (R-3.11).

**Publish** *(publier)* — Right. Make a node, its name and all its content public.

**Read** *(lire)* — Right. Read the content of a file.

**Receipt** *(reçu)* — Entry attested by its author and bearing the fingerprint of the state of the journal at that instant, of which the author keeps a copy.

**Record** *(enregistrement)* — The public and anonymous view of a node: what anyone sees of it without any access (R-10.1). Distinct from the node, which it neither names nor situates.

**Recorded act** *(acte inscrit)* — Any action producing an entry in the journal: creations, renamings, edits, deletions, emptyings, publications, closings, re-sealings, mandate issuances, revocations, countersignatures (R-8.2).

**Rename** *(renommer)* — Right. Change the name of a file or a folder contained in a folder.

**Re-seal** *(re-sceller)* — Act. Close the future of a subtree upon the current mandates alone, without changing anything else; who may do so, and what it closes, is fixed by R-7.11 (X-27).

**Revision** *(révision)* — Counter of a node's modifications; what it counts is fixed by R-2.8.

**Revocation** *(révocation)* — Act putting an end to a mandate.

**Right** *(droit)* — One of the ten permissions a mandate may bear: *browse*, *add*, *rename*, *delete*, *read*, *edit*, *audit*, *publish*, *close*, *delegate*.

**Root** *(racine)* — The node that contains the whole Ethos.

**Scope** *(périmètre)* — The node a mandate bears upon, and everything that node contains.

**Seal** *(sceau)* — The tie of each entry to the one that precedes it, which makes the journal a sequence.

**Sub-delegate** *(sous-délégué)* — Delegate whose mandate was issued by another delegate.

**Test** *(éprouver)* — To know, without reading anything else: of a name, whether it is already borne within a folder (R-2.4); of a learned identifier, whether it designates a node of a scope (X-30).

**Third party** *(tiers)* — Anyone who holds neither ownership of the Ethos, nor any current mandate.

**Tree** *(arbre)* — The set of the nodes of an Ethos: the root, and everything it contains.

**Uniqueness mark** *(marque d'unicité)* — String of characters appended to the proposed name at creation (R-2.1).

**Void** *(sans effet)* — Said of an entry recorded contrary to the rules: what becomes of it is fixed by R-8.15 and R-9.10.

**Witness** *(témoin)* — Holder of a receipt.

**Write** *(écriture)* — Act that changes the state of the tree: creation, renaming, edit, deletion, emptying, publication, closing.
