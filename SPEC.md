# Aithos — Business specification

**Protocol V1 · document v3.6 · 2026-08-17**


This document specifies the core protocol, alone.

## 0\. Preamble

Aithos is a protocol. A vault that follows it is an **Ethos**: a tree of nodes, a journal, mandates. This document is a **business specification**: it describes use cases and observable guarantees, never mechanisms. The terms are defined in the glossary (attached document), which contains no rule.


**The adversary model.** The guarantees of this document hold whatever tool is employed, including against whoever uses no conformant tool and examines the Ethos directly. No third party stands between the actors and the Ethos. What could be learned while one could learn it is deemed learned: no guarantee promises to make it forgotten (R-1.12).


**The medium.** Anyone may hold the medium of the Ethos, including a hostile actor. The protocol promises neither availability nor conservation: whoever holds the medium can destroy it or render it inaccessible; he can alter nothing in it without that being seen — by whoever can confront with the journal what is altered, or holds an external reference point —, within the limits that Part I specifies (R-1.10, X-17, X-18). An Ethos exists in a single copy, which governs — the actors, for their part, act from as many machines as they want.


**How to read this document.**


  - **Part I** states the rules. Each rule bears a stable identifier — R-*section*.*number* for rules, X-*number* for assumed limits — and is normative only at that place.
  - **Part II** restates the same truths through the eyes of each role. It states no new rule: each statement cites its sources in brackets, and in case of divergence, the rule governs.
  - **Annex A** cross-tabulates roles, acts and states; each cell refers to a rule or to an assumed limit.


**Markers.** ✅ possible or guaranteed · ❌ refused or impossible · ◆ fact — a property or a limit, assumed, that grants nothing and refuses nothing.

# Part I — The rules

## 1\. The Ethos

  - **R-1.1** ✅ An Ethos is created from an identity, whether existing or created for the occasion. The protocol does not produce identities.
  - **R-1.2** ✅ An Ethos bears an identity of its own, fixed at its creation and derived from the act of creation, distinct from the identity of its owner.
  - **R-1.3** ✅ The Ethos identity and the public mark of its owner are readable by anyone.
  - **R-1.4** ✅ One same identity may be the owner of several Ethoses. They have no link of state between them — nothing is shared: neither journal, nor mandates, nor receipts — and bear distinct Ethos identities. ◆ Their common owner, for his part, is public (R-1.3): whoever sees the two links them by his mark. Whoever wants publicly separate vaults employs distinct identities.
  - **R-1.5** ✅ One same identity may be the owner of one Ethos and the holder of a mandate in another.
  - **R-1.6** ❌ No one other than the owner can create an Ethos bearing the identity of an existing Ethos.
  - **R-1.7** ✅ On a medium that serializes, recorded acts are so recorded: one at a time. Two recordings cannot be simultaneous, from whomever they come; the second waits or fails. ◆ This serialization is the only promise of the document that depends on the medium: the protocol cannot impose it on a hostile medium (R-1.10). Such a medium can hold two competing runs of the same journal — an equivocation — and present to each his own: the equivocation is not prevented, it is detectable after the fact, by confrontation of receipts (X-17, R-9.5, R-9.9). On a medium under a single physical holding (R-1.9), serialization goes without saying.
  - **R-1.8** ❌ The root can be neither deleted, nor renamed: it has no parent, hence no name. ✅ It can be emptied of all its content.
  - **R-1.9** ✅ An Ethos exists in a single copy, which governs; there is no synchronization between copies — an equivocation of the medium does not create a second one: it is a fraud, detectable by receipts (R-1.7, X-17). ◆ The same identity employed from several machines remains a single identity, and the journal does not distinguish machines.
  - **R-1.10** ◆ Anyone may hold the medium of the Ethos, including a hostile actor. Whoever holds the medium reads in it only what a third party reads (section 10), and can alter nothing in it without that being seen — by whoever can confront with the journal what is altered, or holds an external reference point (R-8.8, R-8.14, X-18). What he can always do: destroy it, render it inaccessible, or hold an equivocation, detectable by receipts (X-17, R-1.7).
  - **R-1.11** ✅ The owner exercises over the whole Ethos, without holding a mandate, everything a mandate can grant: he is at the origin of all the chains. His limits are those of the protocol, never those of a scope. ◆ No act of another can exclude him: every switch of the held form — creation, publication, closing, re-sealing — includes him as a matter of course, without his having anything to do or anything to receive (R-3.9, R-7.11).
  - **R-1.12** ◆ What anyone could learn while he could learn it — through an access, or because it was public — is deemed learned: no guarantee promises to make him forget it, and every rule that withdraws or closes an access is to be understood in this way (R-7.3, R-3.8, X-9, X-10).
  - **R-1.13** ✅ The Ethos identity names, from creation onward, the methods that hold it. ◆ V1 defines neither rotation nor migration of these methods: this naming is an anchor point, reserved for a later version (X-22).

## 2\. The nodes and their names

  - **R-2.1** ✅ To create a node is to propose a name; a uniqueness mark is adjoined to it. The uniqueness mark is verifiable by anyone who can read the name it completes (R-8.7): tied to the creation entry, whose place is unique and public, it can be neither chosen, nor reused — by no one — nor foreseen by whoever does not record the entry. No creation ever fails on account of a name already taken, and no one — not even the owner — can impose an exact name at creation. A creation does not teach its author that a name already exists.
  - **R-2.2** ✅ The creator of a node knows the effective name of what he has created.
  - **R-2.3** ✅ A name can be changed afterwards by whoever holds *rename* on the folder that bears it.
  - **R-2.4** ❌ A renaming cannot give a name already borne in the same folder: it fails. ✅ So that this failure is observable by the renamer himself, whatever his other rights, every holder of *rename* on a folder can test a name: know whether it is already borne there, without reading anything else — at the price of the oracle that X-12 assumes. ◆ A renaming in collision recorded nonetheless is a void entry (R-8.15).
  - **R-2.5** ❌ A node cannot be moved. ✅ The effect of a move is obtained in three acts — read at the origin, create at the destination, delete at the origin — and the created node is a new node, without the mandates of the old one; toward greater depth, the creation remains held by the bound (R-2.6).
  - **R-2.6** ✅ Each node bears a stable and opaque identifier, which is not guessed: no one can enumerate it, nor derive it from anything readable — whoever has not received it cannot produce it (R-10.2). ◆ The identifier of a private node is read nowhere outside the mandates that designate it (R-6.10, R-8.5) and the entries readable with *audit* (R-8.7): it is received — by mandate, by *browse*, by creation — or learned through another channel. ◆ The identifier of a public node is no more readable from its public surface: the public index delivers the names (R-3.6), never the identifiers — public makes readable, not designatable. It is received like that of a private node; observing a public period therefore does not acquire the tracking of X-29 — apart from what the correlation of X-21 delivers to the continuous observer. ❌ The tree does not exceed thirty-two levels: a creation beyond that is a void entry (R-8.15). ◆ Identifiers and designations are of one same size whatever the depth, and nothing, in the public view of a node (R-10.1), grows with it: neither an identifier nor a record lets anything of the depth be read (X-30).
  - **R-2.7** ✅ Each node bears, readable by anyone who can read that node, its author's mark: the identity of its last author, the mandate invoked, and the unforgeable attestation that the present state is indeed the one this author wrote. ◆ From this attestation, a reader alone verifies the author and the integrity of the state presented to him; that this state is the last one is verified by confronting it with the last valid entry bearing its fingerprint — with *audit* (R-8.6) — or by an external reference point (X-18): a hostile medium can serve an authentic earlier state (R-1.10, R-8.14).
  - **R-2.8** ✅ An Ethos keeps only the current state. The revision of a node counts the valid entries bearing a fingerprint of it (R-8.6) — publication, closing, re-sealing and its own renaming do not count therein — without giving access to any earlier state, and no earlier version of any file content is accessible. ◆ The history of the names, for its part, remains in the journal (R-8.3, R-8.4, X-11).

## 3\. Private and public

  - **R-3.1** ✅ A node is private or public. Private is the default.
  - **R-3.2** ✅ That a node is private or public is observable by anyone, without any access: the content of a public node is in the clear, and is read with any tool.
  - **R-3.3** ✅ A created node inherits the visibility of its parent. A node is public if its parent is public. ❌ A private node cannot exist in a public node.
  - **R-3.4** ✅ A public node can exist in a private node.
  - **R-3.5** ✅ Publishing a private node makes public that node, its name and all its content, present and to come. That node becomes the publication point. ❌ A node that is already public cannot be published: there exists only one publication point per public subtree.
  - **R-3.6** ✅ The name of a public node is readable by anyone, although it remains borne by the index of its parent. The index of a public folder is public: the names of what it contains are readable by all. ◆ As long as the parent of the publication point is private, a public name therefore reveals an element of a private index. This leak is assumed.
  - **R-3.7** ✅ Renaming a public node is an act like any other; the new name is public as long as the node is.
  - **R-3.8** ✅ Closing is done at the publication point, and makes private the node, its name and all its content. The name returns to the sole index of its parent. ❌ A part of a public subtree cannot be closed alone: it would become a private node in a public node, which is impossible (R-3.3). To keep only a part of it public, one closes the whole and publishes the desired part.
  - **R-3.9** ✅ Closing restores access to every holder of a mandate on the closed node, on one of its descendants, or on a node above. None of them has anything to do or anything to receive. ◆ If earlier revocations have not yet been followed by a re-sealing, the access thus restored may be fully effective only after re-sealing — the same regime as the exclusion of the revoked party (R-7.11, X-27). ◆ Restoring access to the mandates from above, which the closer cannot enumerate (X-8), is done without knowing them: the closed subtree blindly reattaches to what contains it — an assumed property, of the same order as those of R-8.16.
  - **R-3.10** ✅ Publishing and closing modify, cancel and suspend no mandate. The mandates remain in force during the publication and recover their full effect at the closing.
  - **R-3.11** ✅ Publishing a private node whose content already includes publication points (R-3.4) absorbs them: the published node becomes the sole publication point of the subtree (R-3.5), and the publication entry also touches the absorbed points, which it designates (R-8.4) — this designation is read by portions, with *audit* (R-8.7); it is not public — the publication entry bearing no fingerprint (R-8.4), the public metadata of the absorbed points do not move thereby (R-10.1, X-21): nothing designates them to whoever does not have *audit*. The absorbed nodes remain public; only their quality as a point is extinguished — their entries remain (R-8.12). ❌ An absorbed node can no longer be closed separately: closing is done at the outermost point, the only one (R-3.8), and makes private the whole subtree, formerly absorbed nodes included; republishing them is a new act.
  - **R-3.12** ✅ Closing changes the form under which the Ethos holds the contents and the names of the whole closed subtree: what was read without access during the publication is read no longer, whatever the tool: Private is understood at the moment one learns, not at the moment one closes. ◆ What was retained during the public period — a copied content, or the means to re-read the public form of that time — remains acquired (R-1.12): nothing recovers it (X-10). ◆ The holder of *close* thereby manipulates a content he could already read, since public (X-4). ◆ The cost of the closing is proportional to the volume closed, and that of the publication to the volume published: the one and the other change the form under which the Ethos holds an entire subtree.

## 4\. The identities

  - **R-4.1** ❌ An identity can be neither renamed, nor transferred, nor have its secret changed.
  - **R-4.2** ❌ The identity of the owner cannot change after the creation of the Ethos. There is neither transmission, nor succession, nor recovery.

## 5\. The ten rights

  - **R-5.1** ✅ A mandate grants all or part of these ten rights, and nothing else. They are **primitive, independent and cumulative**: a mandate bears as many of them as one wants, and none implies another.


  - **R-5.2** The ten rights:


  - ✅ **browse** — see the index of a folder and of everything it contains, at any depth: the names, the sizes, the dates, the revisions, the visibility — and, in so doing, know the identifiers.


  - ✅ **add** — create a file or a folder in a folder.


  - ✅ **rename** — change the name of a file or a folder contained in a folder.


  - ✅ **delete** — delete a file or a folder contained in a folder, or empty a folder.


  - ✅ **read** — read the content of a file.


  - ✅ **edit** — replace the content of a file.


  - ✅ **audit** — read the journal of a node and of its content: the acts, their authors, the identifiers of the nodes concerned, the mandates invoked, the fingerprints.


  - ✅ **publish** — make public a node, its name and all its content.


  - ✅ **close** — make private a node, its name and all its content.


  - ✅ **delegate** — issue a mandate that does not exceed one's own.


  - **R-5.3** ✅ What is not a right: **revoke** — a consequence of the place in the chain (R-7.1); **re-seal** — a consequence of the power to revoke (R-7.11); **countersign** — every holder can do it, whatever his rights (R-9.2); **verify** — anyone can do it, within the limit of what he sees (R-8.14); **record in the journal** — the journal writes itself, at each act (R-8.2).


  - **R-5.4** ✅ Acting on a node presupposes knowing it, that is, knowing its identifier. ❌ A right exercised on a node whose identifier one does not know remains void.


  - **R-5.5** ✅ What makes a node known: *browse*, for the whole scope; the mandate, for its own scope; creation, for the created node; and any other channel — an identifier learned elsewhere counts as knowledge, and the right is exercised on it normally. ◆ Whoever acts on an identifier learned elsewhere attests a coverage he has not seen — his rights may let him test it (X-30): if he records without testing and the node is outside his scope, his entry is void (R-8.15).


  - **R-5.6** ◆ The protocol does not hide a node from whoever holds the right and the identifier. It simply does not supply the identifier.


  - **R-5.7** ✅ The rights combine freely, in any set whatever. A derived mandate that includes *delegate* allows its holder to delegate in his turn; a mandate that does not include it is a terminus. **It is for the issuer of a mandate to combine the primitive rights to obtain the effect he wants.** The protocol combines nothing in his stead and corrects no combination.


  - **R-5.8** ❌ On a mandate whose scope is a file, *browse*, *add*, *rename* and *delete* have no effect: a file contains nothing. ❌ On a mandate whose scope is a folder, any right without *browse* is exercised only on the folder itself, on what one has created in it, or on a node known otherwise (R-5.5).


  - **R-5.9** ✅ What each operation requires: creating a node — *add* on the parent folder; renaming a node — *rename* on its parent; deleting a node — *delete* on its parent; emptying a folder — *delete* on that folder itself, without designating anything of what it contains; replacing the content of a file in its entirety — *edit* on that file; modifying only a part of it — *read* and *edit*; publishing — *publish* on the node; closing — *close* on the publication point; reading the whole journal — *audit* on the root; seeing all the names — *browse* on the root.


  - **R-5.10** ✅ Certain acts bear as a matter of course on a whole subtree, without their author having to know its content: deleting a folder, emptying, publishing, closing, re-sealing. Revocation, for its part, is not one of them: it is recorded at a point (R-6.5) — its cascade bears on mandates, not on nodes (R-7.2), and the traversal of the subtree belongs to the re-sealing (R-7.11). ◆ To bear as a matter of course on a whole subtree, the act traverses its shape: its author knows its anonymous extent — never the names nor the contents (X-25).


  - **R-5.11** ✅ Deleting a folder, emptying, publishing, closing, revoking and re-sealing never leave an intermediate state: as long as the act is not recorded, nothing has changed; as soon as it is, everything has changed. ◆ No duration is promised, nor any instantaneity: only the absence of an intermediate state is.
## 6\. The mandate

  - **R-6.1** ✅ A mandate grants rights over a scope to an identity, designated by its public mark. The scope is a node — a folder, or a single file — and the mandate applies to it and to everything it contains, at any depth.
  - **R-6.2** ✅ A mandate bears on a node, never on a name: renaming the scope changes nothing about the mandate. ❌ One can delegate neither "all files of such-and-such type", nor "this folder except such-and-such part". For a thing to be delegable separately, it must be a separate node.
  - **R-6.3** ✅ The beneficiary has nothing to do and nothing to receive outside the Ethos: his access is recorded there, and he finds it there.
  - **R-6.4** ✅ Several mandates may coexist on the same node, for the same identity or for different identities. For one and the same identity, the rights add up, and a mandate on a node accumulates with a broader mandate on one of its parents — yet each act invokes only a single mandate, which must cover it on its own, and an operation requiring two rights (R-5.9) requires both of them from the invoked link (R-8.3, R-6.7). Mandates on disjoint nodes are independent.
  - **R-6.5** ✅ Issuing a mandate alters no content, and revoking it does not either: both the one and the other are recorded at a single point, whatever the scope.
  - **R-6.6** ❌ A mandate has no end date. ❌ A mandate cannot be modified after issuance: one revokes, and one re-issues.
  - **R-6.7** ✅ An issuance by a holder invokes a single link — the owner, for his part, invokes nothing (R-8.3, R-1.11): the rights and the scope of the issued mandate are assessed against that single link, which becomes its parent — never against the union of the issuer's mandates (R-6.4). ❌ A derived mandate cannot exceed its parent, neither in scope, nor in rights. ❌ Issuing a mandate requires *delegate* — borne by the invoked link. ❌ A chain cannot exceed thirty-two links.
  - **R-6.8** ❌ No one may issue a mandate to the owner's identity — not even the owner: he is never a holder, and an issuance that would target him is a void entry (R-8.15). ✅ A holder, for his part, may issue to any other identity, including his own; such a mandate descends from his own and gives him nothing that he does not already hold.
  - **R-6.9** ✅ Every right is delegated on a public node as on a private node, *read* included. Without effect of its own so long as the node is public — everyone reads —, a read mandate takes effect there upon the closing.
  - **R-6.10** ✅ A mandate is readable by anyone, in full: issuer, beneficiary, rights, scope designated by its identifier, place in its chain — and its end, if it has ended.
  - **R-6.11** ◆ Chains reveal the nesting of their scopes: anyone sees the nesting that their issuers attest — such-and-such identifier contained in such-and-such other; a false attestation renders the issuance void (R-8.15), without distinguishing itself publicly (X-26). They reveal neither the names, nor the contents, nor anything else of those nodes.
  - **R-6.12** ✅ The formal validity of a chain is verified on the chain alone, by anyone: each link in it is attested by its issuer, rights and scopes nested, from the owner down to the holder. ◆ Formal, because the nesting of the scopes is attested, never verified by all (R-6.11): a formally valid chain one of whose attestations is false carries void issuances (R-8.15), without the chain alone showing it — only one who has sight of the scopes in question observes it (X-26).

## 7\. The end of a mandate

  - **R-7.1** ✅ The following may revoke a mandate: its issuer; any holder of a link from which it descends, whether he is the issuer of it or not; and the owner, who is at the origin of all the chains. ❌ No one else — not even on his own scope.



  - **R-7.2** ✅ Revoking a mandate revokes all the mandates that descend from it — those that its holder has issued, and those that his sub-delegates have issued in their turn — and those alone.



  - **R-7.3** ✅ A revocation takes effect upon its recording, with no intermediate state (R-5.11). It marks a before and an after:



  - **the revoked party writes no more** — any subsequent attempted act is invalid: recorded all the same, it is a void entry (R-8.15), rejected by anyone who verifies; without delay, without exception. His earlier writes remain valid, attributed and traced;



  - **the future escapes him entirely** — everything that is modified or created after the revocation belongs to the after, out of reach of all the previously revoked parties, content and name, and of it he sees only what a third party sees; this closure is immediate everywhere the writer can situate the revocation, and complete, everywhere, as of the re-sealing of the scope (R-7.11, X-27);



  - **the unchanged past is not taken back** — what already existed and has not changed is deemed known to him, and readable by him so long as it is not replaced. This is not a flaw: he had access to it, and could extract it.



  - **R-7.4** ✅ The revocation withdraws the mandate in full: there exists no revocation of a part of the rights.



  - **R-7.5** ✅ All the other mandates remain what they are, whatever their issuer, whether they bear on the same scope, on a node that it contains, or on a node above. Their holders retain their access without having to intervene or to receive anything.



  - **R-7.6** ❌ A revocation withdraws access to no public node: it remains readable by all, including by the revoked party.



  - **R-7.7** ✅ The former auditor follows the same line: he retains the reading of the journal entries prior to his revocation; of the subsequent entries, he sees only what a third party sees (R-8.7) — completely, at the latest as of the re-sealing (R-7.11, X-27).



  - **R-7.8** ✅ Deleting a node deletes all the mandates that bore on it or on what it contained, whatever their issuer. A deleted mandate disappears from the Ethos; its existence and its disappearance remain recorded in the journal. ✅ The entry of the deletion — or of the emptying — that carries away mandates publicly designates each of the mandates carried away: the end of a mandate is readable by anyone, like the mandate itself (R-6.10). ◆ This mention reveals that the scope of the mandate carried away was located in the deleted node — and, if the mandate bore on that very node, that this node has disappeared. It is one more nesting made public, assumed (R-10.3, R-10.4).



  - **R-7.9** ✅ Emptying a folder deletes no mandate bearing on that folder itself: it remains valid, and what will be created in it thereafter will be accessible to its holder. The mandates that bore on what it contained disappear (R-7.8).



  - **R-7.10** ❌ A mandate ends only by revocation or by disappearance of its scope. There exists no other way to lose one: no mandate is extinguished by time or by inaction.



  - **R-7.11** ✅ After a revocation, the scope of the revoked mandate may be re-sealed: an act of maintenance that closes the future of the subtree upon the current mandates alone, without changing anything else — neither content, nor name, nor visibility, nor mandate. The following may re-seal: the issuer of the revoked mandate, any holder of a link from which that mandate descended, and the owner — the same ones who could revoke (R-7.1), and who hold at least the rights that the revoked party had (R-6.7). The act bears as a matter of course on the whole subtree (R-5.10), with no intermediate state (R-5.11); its entry touches the re-sealed node and bears no fingerprint (R-8.4): public dates and revisions do not move on account of it (R-10.1), and nothing designates the re-sealed scope in the public view — the holder of the medium, for his part, can circumscribe it (X-24). As of its recording, the exclusion of the previously revoked parties is complete for everything that is written thereafter (X-27); the unchanged past remains what it is (R-7.3). ◆ Its cost is proportional to the volume re-sealed.

## 8\. The journal

  - **R-8.1** ✅ There is a single journal for the whole Ethos, in append only. There exists no journal per node.
  - **R-8.2** ✅ Every recorded act produces an entry, including those of the owner. The following are recorded: creations, renamings, edits, deletions, emptyings, publications, closings, re-sealings, mandate issuances, revocations and countersignatures.
  - **R-8.3** ✅ An entry bears: its author; the action; the invoked mandate — the owner invokes none; a declared date; its seal to the preceding entry; and the node or nodes that the act touches, each by its identifier and by its name at the moment of the act — the root, which has no name, by its identifier alone. The name borne by an entry is the one that the index bore at the moment of the act, whether the author of the act was able to read it or not.
  - **R-8.4** ✅ What each act touches, and the fingerprints that its entry bears: *edit* touches the file, and the entry bears the fingerprint of its content after the act; *create* touches the created node and the index of the folder that receives it, with their two fingerprints; *rename* touches the renamed node and the index of its folder — the entry bears the old and the new name of the node, and the fingerprint of the index after the act; *delete* touches the deleted node and the index of its folder, with the fingerprint of the index after the act; *empty* touches the emptied folder and each of the removed nodes, with the fingerprint of the index — empty — after the act; *publish* touches the publication point and, if there are any, the absorbed points (R-3.11); *close* touches the publication point; *re-seal* touches the re-sealed node (R-7.11); none of the three changes any content — their entry bears no fingerprint. The fingerprint of a folder bears on its index. ◆ The asymmetry between emptying and deleting is intended: after an emptying, mandates on the folder subsist (R-7.9) and their auditors need the portion of each removed node (R-8.7); after the deletion of a folder, no interior mandate subsists (R-7.8), and the creation entries suffice for the audit from above.
  - **R-8.5** ✅ Three acts touch no node, and their entries are readable by anyone, in full: the **issuance**, whose entry bears the issued mandate — beneficiary, rights, scope (R-6.10); the **revocation**, whose entry designates the revoked mandate; the **countersignature**, which targets no node and bears the fingerprint of no node — it attests the state of the journal (R-9.1).
  - **R-8.6** ✅ The entries that concern a node are those that touch it (R-8.4). A folder is therefore concerned by everything that modifies its index: creations, renamings, deletions and emptyings within it. ✅ The last valid entry bearing a fingerprint of a node attests its current state: this is how a node is audited; the entries that concern it without bearing a fingerprint of it — publication, closing, re-sealing, its renaming — do not count there, and a void entry attests nothing (R-8.15).
  - **R-8.7** ✅ Who sees what of an entry: **without any access** — its place in the order, its seal to the preceding one, the identity of its author, attested in an unforgeable manner, and its declared date; **with** ***audit*** **on a concerned node** — the action, the invoked mandate, and the portion of the entry that concerns that node: its identifier and the fingerprints that target it; **with** ***browse*** **in addition** — the names borne by that portion, such as at the moment of the act. ✅ An entry that touches several nodes is thus read by portions: each auditor sees those of his scope — the action, the author and the mandate are common to all. ❌ Without *audit*, one reads of an entry neither the identifiers, nor the names, nor the fingerprints that it bears, nor the invoked mandate — apart from what R-8.5 and R-7.8 make public. ◆ The correlation of the public metadata (X-21) nevertheless makes known to anyone, for the entries that bear a fingerprint of a node or change its state, the record or records that they cause to move, and the type of act. ◆ The fingerprints are therefore readable only by the auditors of the concerned node.
  - **R-8.8** ✅ Removing an entry that others have followed, inserting one, or changing the order of the entries is detectable by anyone, without any access, and renders the Ethos corrupted (R-8.14): it is no longer the continuation of its own past. ◆ Removing the last entries while bringing the tree back to the corresponding state, for its part, gives back an Ethos that was coherent: that is the rollback, undetectable without an external reference point (X-18).
  - **R-8.9** ✅ Recording an entry does not require being able to read the journal: every actor who has the right to act has the right to record, even if he can read nothing of the existing entries.
  - **R-8.10** ✅ The date of an entry is declared by its author, attested by him, and can no longer change thereafter. ✅ It cannot be earlier than that of the last valid entry that precedes it — a void entry does not move this bound (R-8.15): the dates of the valid entries never go backwards. ◆ The journal proves the order and the declaration, never the exactness of a date (X-2).
  - **R-8.11** ✅ No reading is recorded in the journal. The journal attests what has been written, never what has been consulted.
  - **R-8.12** ✅ Deleting a node, or the folder that contained it, deletes no entry concerning it. The journal never diminishes: entirely emptying an Ethos does not reduce its journal.
  - **R-8.13** ❌ A node whose journal entry is missing is invalid. Observing this after the fact requires *audit* on that node; a record that appears with no entry at all, under the eyes of a continuous observer, falls under the corrupted artifact, observable without access (R-8.14, X-21).
  - **R-8.14** ✅ Anyone may verify, within the limit of what he sees: that each thing bears the mark of its author, that the mandate chains are formally valid (R-6.12), that the journal is continuous (R-8.8), that the tree is coherent. Corrupted is the Ethos whose artifact has been altered outside the protocol: entry removed, inserted or reordered (R-8.8), false author's mark, state of the tree not corresponding to the journal. Its verification fails, for anyone who conducts it with sight of the altered portion (R-8.13, X-18). ◆ A void entry does not corrupt the Ethos: the invalid act and the corrupted artifact are two distinct regimes (R-8.15).
  - **R-8.15** ✅ An entry recorded contrary to the rules — notably: act outside a mandate, act of a revoked party, renaming in collision, issuance to the owner's identity, issuance exceeding the invoked link in rights or in scope (R-6.7), issuance upon a false nesting attestation (R-6.11), chain beyond thirty-two links (R-6.7), date that goes backwards (R-8.10), creation beyond the depth bound (R-2.6), write of an expired epoch — recorded for an epoch that a re-sealing has closed (R-7.11, X-27) — remains in the journal: it counts for the continuity, by its place and its seal, and the subsequent entries chain onto it without losing any of their validity. It is void: the state of the tree and of the mandates is the one that the valid entries alone produce, and anyone who sees it rejects it, within the limit of what he sees (R-8.14). ◆ Observing the invalidity may require *audit*; one who does not hold it takes the entry for what he sees of it (R-8.7, X-26).
  - **R-8.16** ◆ An entry bears names and designates nodes without its author having to know them (R-8.3, R-8.4): emptying touches each of the removed nodes without the emptier having read either name or content (X-5, X-25); renaming bears an old name that the renamer may not know; creating records a name in an index that the creator cannot browse (R-5.8). Four properties, assumed, make this tenable: the entry references the state of the indexes and of the nodes such as it is, without requiring of its author what he cannot see; it never states in the clear what R-10.3 keeps silent — only the rights of R-8.7 unfold it; writing a name into an index delivers nothing of it to the writer; and the fingerprint of an index is established on the held form — computable by every writer of the folder without reading anything (R-8.9), verifiable by its auditors (R-8.7), closed to the holder of the medium (X-20) — and confirms no name: the name oracle remains the one of X-12, reserved to *rename* (X-19). A tool that does not offer these four properties produces entries contrary to R-8.3, to R-10.3 or to R-8.7: they are void (R-8.15), and the tool is not conformant.

## 9\. The receipt

  - **R-9.1** ✅ Every entry seals the state of the journal up to itself. Every recorded act is therefore worth a receipt to its author: the proof, which he keeps, that the journal passed through there. He has nothing to request and nothing to activate.
  - **R-9.2** ✅ Every holder can obtain a receipt without modifying anything else, by countersigning the state of the journal. ❌ The owner does not countersign: he holds no mandate (R-6.8). His own acts are worth a receipt to him (R-9.1).
  - **R-9.3** ✅ The receipt is self-supporting: verifying it — its form, and the attestation of its author — requires neither access, nor Ethos, nor owner. ◆ Self-supporting is to be understood of the attestation: that the frozen state really was a journal is established only before the Ethos, by proof of continuation (R-9.6, R-9.4). ❌ A receipt reveals of the Ethos neither content, nor name, nor structure, nor node size. ◆ It carries what is needed for it to be verified, and reveals it: the identity of the Ethos, the frozen place — hence the length of the journal at that instant —, the identity of its author and its declared date. To publish a receipt is to say: I acted in this Ethos, at this instant. ✅ It is kept indefinitely, is transmitted and is published freely.
  - **R-9.4** ✅ A receipt becomes binding as soon as an entry of an identity other than that of the owner attaches to it — be it from the author of the receipt himself, and whatever the status of its author at the moment of the recording (R-8.15). ❌ A receipt that nothing else has followed, or that only entries of the owner have followed, is not binding.
  - **R-9.5** ✅ Two receipts are compatible if one is the continuation of the other, or if they freeze the same instant identically. What is comparable outside the presence of the Ethos: two receipts freezing the same instant — if they differ, there is fraud, and anyone observes it, without access. ◆ This observation proves the fraud, never its author: equivocation of the medium or lying receipt (R-9.3), nothing separates them outside the presence of the Ethos — before it, the proof of continuation establishes only which of the two receipts the presented state continues (R-9.6), without separating, by itself, the equivocation from the lying receipt. ❌ For two different instants, nothing is established outside the presence of the Ethos without proof of continuation.
  - **R-9.6** ✅ Whoever holds the Ethos can produce, towards any holder of a receipt, the proof that the Ethos is the continuation of the state frozen by that receipt; anyone can verify this proof alone, without access. ❌ This proof is not produced without the Ethos.
  - **R-9.7** ❌ A receipt can be neither withdrawn, nor cancelled, nor invalidated by the owner. The revocation of its author changes nothing of its force.
  - **R-9.8** ◆ Since a read records nothing (R-8.11), it produces no receipt.
  - **R-9.9** ◆ Whoever produces two histories leaves two incompatible receipts in the hands of their authors — observable within the limits of R-9.5. And no one can set aside the entry of another identity without setting aside everything that has been built upon it (R-8.8).
  - **R-9.10** ✅ A receipt freezes the run of the journal, void entries included: it attests that the journal passed through there, never the validity of the acts it freezes (R-8.15). A void entry of an identity other than that of the owner counts for bindingness (R-9.4): it is in the journal, attested by its author — holder, revoked or anyone —, and no one can set it aside without setting aside what follows.

## 10\. What is always seen, what is never seen

These rules assume leaks. They state what anyone — third party, holder of the medium, former holder — sees permanently, and what no one ever sees.

  - **R-10.1** ✅ Anyone can count the nodes of the Ethos, distinguish them, and read of each one the size, the date of last modification, the revision, and the state — private or public. The public date of last modification and the public revision count only the valid entries carrying a fingerprint of the node (R-8.6, R-2.8): publication, closing, re-sealing and renaming of the node do not appear there. ◆ This view is declarative: held by the writers as the acts proceed, exact if the entries are valid, it is verified with *audit* — not without (X-28). ◆ He sees of it neither the identifier, nor the name, nor the place, nor the content if it is private: nodes, without knowing which ones — but the activity of each record is followed over time, and correlates with the entries (X-21).
  - **R-10.2** ✅ Whoever knows the identifier of a node can recognize it among them, and see of it those same things — never its name nor its content, if it is private, without the rights that give them. ◆ This power does not expire (X-29).
  - **R-10.3** ◆ The private shape of the tree is not seen: apart from the public subtrees (R-3.6), the nestings that the chains reveal (R-6.11), those that the mandates carried away by a deletion reveal (R-7.8) and those that the correlation of public metadata yields as the writes proceed (X-21), no one sees which node contains which other, nor where a node attaches. What has never changed under the eyes of an observer has yielded him nothing.
  - **R-10.4** ◆ Recapitulation of what is permanently public — each point holds from a cited rule: the identity of the Ethos and the public mark of the owner (R-1.3); all the mandates, in their entirety, and their ends — by revocation or by disappearance of the scope (R-6.10, R-8.5, R-7.8); the chains, their formal validity and their attested nestings (R-6.11, R-6.12); the public portion of each entry and the public entries (R-8.7, R-8.5); the public nodes — names, contents, indexes (R-3.2, R-3.6); the count, the sizes, the dates, the revisions, the states (R-10.1); the activity graph, by correlation of the public metadata and the entries, and the anonymous containment that it draws as the writes proceed (X-21). The access graph is therefore public — who holds which rights over which identifiers is permanently seen, by anyone, in the sense of the recorded mandates: an access covertly maintained by a disloyal re-sealer or closer does not figure there (X-27, X-7) — and the activity graph is public too: who moved which records, when, by which type of act.
  - **R-10.5** ✅ No one ever sees who read what. A read appears nowhere in the Ethos: it is neither traced, nor provable, nor refutable (R-8.11, R-9.8). ◆ The holder of the medium, for his part, sees the reads that reach him go by — never the identity that reads (X-24).

## 11\. What is promised to no one

Each limit below is assumed: none is an oversight. Annex A refers to them as to the rules.

  - **X-1** ◆ No access expires by itself: closing an access is always an act, and a forgotten mandate remains valid indefinitely (R-7.10). A mandate that carries only viewing rights — *browse*, *read*, *audit* — can record only countersignatures: it may produce no act at all, and no one can establish with certainty that it is no longer in use.
  - **X-2** ◆ No date is proved exact: one proves that it was declared thus, and that it does not precede the last valid entry before it (R-8.10) — within the limit of what the verifier sees (R-8.15). An aberrant declared date — far in the future — therefore constrains all those that follow, forever: anyone who can record a valid entry — a pure-viewing holder suffices (R-9.2, X-23) — can thus constrain every later actor, owner included, to declare a date he knows to be inexact, on pain of nullity of his acts (R-8.15). This availability attack is assumed in the core; the order, for its part, never lies. Keeping it at bay falls to the tools, and, beyond that, to a timestamping provider, outside the protocol.
  - **X-3** ◆ A right granted without *browse* acts only on the scope itself, on what its holder has created there, or on an identifier learned elsewhere (R-5.8). The protocol does not signal it.
  - **X-4** ◆ Granting *publish* without *read* does not prevent reading: the holder publishes the node, then reads it like anyone (R-3.2).
  - **X-5** ◆ *delete* alone is a blind total destruction: it allows a scope to be emptied without reading either name or content of it, contents and mandates of others included (R-5.9, R-7.8) — without holding either *delegate* or the power to revoke. The protocol never requires seeing what one destroys; only the anonymous shape of the subtree is known to the act (R-5.10, X-25).
  - **X-6** ◆ Creating, renaming or deleting a specific file requires the corresponding right on its parent folder — hence on all its siblings (R-5.9). The node is the only mesh.
  - **X-7** ◆ Refusing *delegate* does not prevent a holder from letting others read through him, nor from sharing what he has read. It only prevents the access thus consented from being recognized, attributed, or usable for writing. A holder who communicates his identity secret transfers his entire identity: nothing prevents him from doing so, and nothing distinguishes that use from his own (R-4.1).
  - **X-8** ◆ A holder cannot enumerate the nodes above his scope, nor, therefore, all those who have access to it. He knows that whoever has access to a node above has access to his scope; he recognizes, among the mandates — all public (R-6.10) —, those that the public nestings (R-6.11, R-7.8) place above a point of his chain, beginning with those that bear on his own links; every other mandate above him remains indiscernible from the other mandates of the Ethos.
  - **X-9** ◆ Revoking an access does not erase what has already been extracted, and does not take back the reading of what existed and has not changed: what a holder was able to read is acquired for him (R-7.3).
  - **X-10** ◆ Closing a node recovers nothing of what was retained while it was public — a copied content, or the means to re-read the public form of that time (R-3.8, R-3.12) —, and no one knows who read it during that time (R-10.5).
  - **X-11** ◆ An Ethos keeps no earlier version of its file contents (R-2.8). The journal keeps the fingerprints of past states, never the contents themselves — but it carries the names as the acts proceed (R-8.3, R-8.4): the history of the indexes is reconstituted, with *audit* and *browse* (R-8.7).
  - **X-12** ◆ No one can impose an exact name at creation (R-2.1). Renaming, for its part, fails on collision, and this failure must be observable by the renamer without his reading the index (R-2.4): whoever holds *rename* on a folder can therefore test any name of that folder — not only by renaming attempts, but offline, by dictionary, without leaving a trace (R-8.11). This is the analogue, for names, of what X-19 assumes for fingerprints. This name oracle is assumed; it extends neither to the folders that the holder cannot designate (R-5.8), nor to the contents.
  - **X-13** ◆ The loss of the owner's identity secret is the loss of the Ethos; its theft is the taking of control of it: the thief is the owner (R-4.1, R-4.2).
  - **X-14** ◆ A portion of history written by the owner alone, without any other identity having intervened in it — be it by a void entry (R-9.10) —, cannot be proved authentic (R-9.4).
  - **X-15** ◆ The owner who reproduces identically the creation of his Ethos obtains a second Ethos of the same identity. No local mechanism can prevent him from it: he can redo at home everything he has already done. Only the receipts already handed over distinguish the two histories (R-9.5, R-9.9).
  - **X-16** ◆ The protocol does not say how an issuer obtains the public mark of the one he wishes to mandate.
  - **X-17** ◆ Neither availability nor conservation is promised: whoever holds the medium can destroy it or render it inaccessible. He cannot alter anything in it without that being seen — by whoever can confront with the journal what is altered, or holds an external reference point (R-8.8, R-8.14, X-18). He can, finally, maintain an equivocation — two competing runs, each one seeing his own — which nothing prevents and which only the confrontation of receipts reveals (R-1.7, R-9.5, R-9.9).
  - **X-18** ◆ Without a reference point kept outside the Ethos — a receipt, or a fingerprint of the journal noted beforehand —, a return to an earlier state is undetectable: that state was coherent.
  - **X-19** ◆ A fingerprint makes it possible to confirm an exact supposition: a guessable content — a yes, an amount, a name among a hundred — is confirmed by trials. This is why fingerprints are readable only by the auditors of the node concerned (R-8.7).
  - **X-20** ◆ Whoever does not have *audit* sees who acted and when; by the correlation of public metadata (X-21), he moreover attaches each entry to the records that it moves and to the type of act. What remains closed to him: the identifiers carried by the entry, the names, the contents, the fingerprints and the mandate invoked — apart from what R-8.5 and R-7.8 make public (R-8.7). ◆ Closed is not unguessable: the author of an entry is public, his mandates too (R-6.10) — when the identity holds only one of them, the mandate invoked is deduced.
  - **X-21** ◆ Public metadata correlate with the entries: the date of last modification of a node is the declared date of the last valid entry carrying a fingerprint of it, and the revision counts those same entries (R-10.1, R-2.8, R-8.6) — there is no other clock. Whoever observes the Ethos over time therefore pairs entries and node records, and reads the type of act: a record appears — creation —, disappears — deletion —, changes size — edit —, changes state — publication or closing, readable from the private/public bit itself (R-3.2), without a jump of date or of revision (R-8.6). Creation carries the fingerprint of the created node and that of the index of its folder (R-8.4): the two records move at the same entry; deletion and emptying are read from the movement that the fingerprint of the index imprints on the record of the folder, joined with the simultaneous disappearance of the removed records. Hence the nesting of the paired records — creation, deletion and emptying pair the child and its folder; renaming, for its part, moves only the record of the folder, without designating the renamed node — apart from public nodes, whose name changes in the sight of all (R-3.6, R-3.7). The activity graph is public (R-10.4): who moved which records, when, by which type of act; and it draws, step by step, an anonymous containment — that of the only nodes that change (R-10.3). It bears on the anonymous records of R-10.1, never on the names, the contents, the fingerprints nor the mandate invoked.
  - **X-22** ◆ An Ethos is made to last; the methods that hold it belong to a time. Their weakening, and what a patient adversary has archived of the medium while waiting for it (R-1.10), are covered by no guarantee. V1 offers neither rotation of secrets (R-4.1), nor migration: the only way out is a new Ethos, without its journal, its mandates or the force of its receipts. The identity of the Ethos names its methods from the creation onwards (R-1.13): anchor point of a migration that a later version will define.
  - **X-23** ◆ Countersigning is the only valid act that requires no particular right and remains without bound or cost: every holder, even a pure-viewing one, can record countersignatures without limit (R-9.2, R-8.9), and the journal never diminishes (R-8.12). More broadly, nothing materially prevents anyone — a revoked party, a witness, a third party — from recording an entry: it is void, but it remains, counts for the continuity, and freezes the journal like a receipt of passage (R-8.15, R-9.10). The journal can therefore be enlarged without bound by anyone, not only by a holder. A flood of entries does not falsify the canonical state — it enlarges the journal, can blur the public view (X-28), and each entry carries a declared date — without displacing the bound of R-8.10 when it is void: the poisoning of dates remains the affair of valid entries alone (X-2).
  - **X-24** ◆ The holder of the medium observes the accesses that reach him — reads and writes: which regions of the artifact, when, at what rhythm, from where. The protocol renders these accesses neither anonymous, nor indistinguishable; it guarantees only that no identity is attached to the reads and that the Ethos keeps no trace of them (R-1.10, R-8.11, R-10.5). He can in particular circumscribe the extent of a subtree published, closed or re-sealed, whose burst of writes draws the shape — even when nothing designates it in the public view (R-7.11, R-10.3). Whoever reads on a copy already obtained is seen by no one.
  - **X-25** ◆ Exercising an act that bears as a matter of course on a subtree (R-5.10) makes known to its author the anonymous shape of that subtree — how many nodes, how nested — never a name nor a content: it is this traversal, carried by the power that authorizes the act, that makes the act totally blind without a trusted third party. It adds little to what X-21 makes public as the writes proceed; this is assumed. Re-sealing goes further: closing the future again on the current mandates alone pairs the public mandates (R-6.10) with the nodes of the subtree — its author learns which mandates live inside; this is the knowledge that the membership oracle opens, in any case, to whoever covers the scope (X-30).
  - **X-26** ◆ The validity of an entry is not uniformly decidable: validities chain together, and each one holds the state that the only entries he can judge produce (R-8.15, R-8.7). Two observers with different rights can hold, in good faith and for as long as the gap between their views lasts, two different states of the tree. The canonical state — the one that the valid entries alone produce — is objective and unique: the owner, and every observer whose view covers the entries in question, compute it; the others approximate it within the limit of what they see.
  - **X-27** ◆ Revoking closes the future as one writes. Validity, for its part, is immediate: from the recording onwards, the revoked party writes no more (R-7.3). But holding "private" before whoever examines the artifact (R-1.10) presupposes that each new write closes again on the current mandates alone — and a writer cannot always situate a revocation that covers him from higher than his view (R-10.3, X-8). The exclusion of the revoked party over what is written after the revocation is therefore immediate everywhere the writer can situate it — his scope, his creations, his own chain — and complete, everywhere, at the first re-sealing of the revoked scope (R-7.11); until then, a new write can remain within his reach, as the unchanged past already is (R-7.3). The access given back by a closing follows the same regime (R-3.9). The core does not promise immediacy everywhere; a provider can offer it, on top. ◆ The proper execution of a closing or of a re-sealing is attested by its author, never verifiable by all: whoever can re-seal can also covertly maintain the access of the revoked party — as he could, openly, re-issue it (R-7.1, X-7). Making this execution verifiable is a matter for a provider, or for a later version.
  - **X-28** ◆ The public layer is declarative. Sizes, dates, revisions — and the graph that X-21 draws from them — are held as the acts proceed by their writers: their exactness is that of the valid entries, and this validity, precisely, is not judged without *audit* (X-26, R-8.7). A void entry can therefore move the public view of a record without changing anything of the canonical state (R-8.15): dates, revisions and activity graph can be durably blurred for whoever cannot judge — a flood of void entries can do so at will (X-23). Verifying the view requires *audit*; signalling it is a matter for the tool.
  - **X-29** ◆ Whoever has known the identifier of a node follows its record forever: size, date, revision, state (R-10.2), activity (X-21) — even after the end of all access, what is learned being learned (R-1.12), and the identifiers of mandated scopes remaining public forever (R-6.10) — the public surface of a node, for its part, does not yield its identifier (R-2.6). A former holder thus keeps a named handle on the nodes he knew: he knows that such and such a node — whose name he knew, perhaps the content — has just changed, grown, become public. Only the re-creation of the node (R-2.5) breaks the tracking.
  - **X-30** ◆ Every mandate makes of its scope a membership oracle: testing whether an identifier learned elsewhere designates a node of the scope — offline, without trace (R-8.11). A viewing right opens it for certain — the success of a read or of an audit is the test — and every sweeping act carries it (X-25); no mandate is deemed not to open it. This is the analogue, for the place, of the name oracle (X-12) and of that of the fingerprints (X-19): it yields neither name, nor content, nor path — a yes or a no, per identifier tested. It makes observable the coverage that an act on a learned identifier attests (R-5.5), and grounds the pairing of the mandates with the nodes that re-sealing requires (X-25, R-7.11).
# Part II — The perspectives

No new rule here. Each statement derives from the rules of Part I and cites its sources in brackets; in case of divergence, the rule governs. Sections 13 to 16 hold for any holder of a mandate on a node P, whether that mandate comes from the owner or from another delegate \[R-6.1\].

## 12\. Owner

### 12.1 Seeing

  - ✅ I can read the complete index of the Ethos: all the folders, all the files, their true names \[R-1.11, R-5.2\].
  - ✅ I can see, for each node, whether it is private or public \[R-3.2\], its size, its date of last modification, its revision \[R-5.2\].
  - ✅ I can see, for each node, the list of the identities that have access to it, and for each access the complete chain that produced it, from me down to the holder, with for each link the identity and the rights — in the sense of the recorded mandates \[R-6.10, R-6.11, R-6.12, R-10.4, X-27\].
  - ✅ I can see, before revoking a mandate, the exact list of the accesses that will fall with it \[R-7.2, R-6.10\].
  - ✅ I can see all the active mandates; the journal keeps trace of those that have been revoked or deleted \[R-6.10, R-8.5, R-7.8, R-8.12\].
  - ✅ I can see, for each mandate, for how many journal entries it has produced no act \[R-1.11, R-8.7\].
  - ❌ I cannot see who has read what \[R-10.5\].
  - ❌ I cannot establish that a viewing mandate no longer serves: it may produce no act \[X-1\].

### 12.2 Reading

  - ✅ I can read the content of any file, private or public, and browse any folder, at any depth \[R-1.11\].
  - ✅ I can read a public file while holding nothing, with any tool \[R-3.2\].
  - ❌ A private file is read only with my identity secret: without it, I am but a third party \[X-13\].
  - ❌ I cannot read an earlier version of a file: an Ethos keeps only the current state \[R-2.8\].

### 12.3 Adding, editing, deleting

  - ✅ I can create a file or a folder anywhere, modify or replace any file \[R-1.11\].
  - ✅ I can rename or delete any node, except the root \[R-1.11, R-1.8\].
  - ✅ I can empty any folder in a single act, root included \[R-5.9, R-1.8\].
  - ✅ Deleting a node deletes all the mandates that bore on it or on its content, whatever their issuer \[R-7.8\].
  - ✅ Deleting a node erases its content, never its history: its fingerprints and its entries subsist in the journal \[R-8.12, R-8.6\].
  - ✅ I can obtain the effect of a move in three acts; the node created is a new node, without the mandates of the old one — depth bound included \[R-2.5, R-2.6\].
  - ❌ I cannot move a node \[R-2.5\].
  - ❌ I cannot impose an exact name at creation; I can rename afterwards \[R-2.1, R-2.3\].
  - ❌ I cannot create beyond thirty-two levels of depth \[R-2.6\].
  - ❌ I cannot write without recording an entry in the journal \[R-8.2\].
  - ❌ I cannot make a node disappear from the journal by deleting it \[R-8.12\].

### 12.4 Publishing and closing

  - ✅ I can publish any private node, and close a node at its publication point \[R-3.5, R-3.8\].
  - ✅ A public node may remain within a private folder: the state is lawful and durable \[R-3.4\]; as long as the parent of the publication point is private, the public name reveals an element of a private index \[R-3.6\].
  - ✅ Publishing a folder that contained publication points absorbs them: the entry designates them, closing is then done at the outermost point, and the closing makes the whole subtree private — it is up to me to republish what I want to keep public \[R-3.11\].
  - ✅ As soon as the closing is recorded, the form under which the Ethos held contents and names has changed: it is no longer read without access — what was retained from the public period remains acquired \[R-3.8, R-3.12, X-10\] — and all the holders concerned recover their access without receiving anything from me — fully effective at the latest at the re-sealing, if revocations remained to be re-sealed \[R-3.9, R-5.11, X-27\].
  - ✅ The mandates bearing on this node or its content remained in force during the publication and recover their full effect \[R-3.10\].
  - ❌ I cannot know who read the node while it was public \[R-10.5\].
  - ❌ I cannot recover what was copied during that time \[X-10\].

### 12.5 Delegating

  - ✅ I can issue a mandate for the benefit of an identity, designated by its public mark; the beneficiary has nothing to do, he finds his access in the Ethos \[R-6.1, R-6.3\].
  - ✅ I can delegate a folder and all its content, or a file alone \[R-6.1\].
  - ✅ I can delegate any combination of the ten rights, and grant or refuse *delegate* \[R-5.7\].
  - ✅ I can delegate several disjoint nodes to the same identity — several independent mandates — and one same node to several identities \[R-6.4\].
  - ✅ I can delegate any right on a public node, *read* included: a mandate of reading takes effect there at the closing \[R-6.9\].
  - ✅ A mandate bears on a node, never on a name: renaming the scope changes nothing in the mandate; to delegate a thing separately, I make it a separate node \[R-6.2\].
  - ❌ I cannot set an end date, nor modify a mandate after issuance: I revoke and I re-issue \[R-6.6\].
  - ❌ I cannot issue a mandate to my own identity: no one can \[R-6.8\].
  - ◆ The protocol does not warn me that a combination of rights will produce nothing: it is up to me to compose it \[R-5.7, X-3\].
  - ◆ The node is the only mesh: granting *add*, *rename* or *delete* on a folder exposes all the siblings of what is targeted therein \[X-6\].
  - ◆ Refusing *delegate* does not prevent a holder from letting others read through him, nor from sharing what he has read: it only prevents the access thus consented from being recognized, attributed, or usable for writing \[X-7\].
  - ◆ The protocol does not tell me how to obtain the public mark of whoever I want to mandate: it is obtained outside the Ethos \[X-16\].

### 12.6 Revoking

  - ✅ I can revoke any mandate, whatever its issuer: I am at the origin of all the chains \[R-7.1\]. Revoking is not a right that is held: a consequence of the place in the chain \[R-5.3\].
  - ✅ Revoking a mandate revokes all those that descend from it, and those alone \[R-7.2\].
  - ✅ The holders who retain a mandate on the same scope keep their access, without receiving anything from me \[R-7.5\].
  - ✅ After revocation, the former holder no longer writes, and nothing of what changes or comes into being thereafter is accessible to him. What he was able to read and which has not changed is deemed acquired for him \[R-7.3\].
  - ✅ An interrupted revocation leaves the Ethos coherent: either it took place, or it did not take place \[R-5.11\].
  - ✅ I can re-seal the scope of a revoked mandate, whatever its issuer: the exclusion of the revoked party from what will be written thereafter is then complete \[R-7.11, X-27\].
  - ◆ Before this re-sealing, a new write in the former scope may remain within reach of the revoked party, if its author could not situate my revocation \[X-27\].
  - ❌ I cannot recover what the former holder has already extracted, nor take back from them the reading of the unchanged past \[X-9\].
  - ❌ There exists no revocation of a part of the rights \[R-7.4\].
  - ❌ I cannot withdraw by revocation the access to the public nodes of the scope: for that, they must be closed \[R-7.6, R-3.8\].

### 12.7 Journal

  - ✅ There is a single journal for the whole Ethos, append-only \[R-8.1\]; I can read it in its entirety, read-only: who did what, on which node, in what order, under which mandate \[R-1.11, R-8.7\].
  - ✅ I can delegate the reading of the journal on a scope, or on the whole Ethos, without giving access to the contents \[R-5.2, R-5.9, R-5.1\].
  - ✅ I can verify that the journal has not been truncated, added to or reordered — except for the removal of the last entries, undetectable without an external reference point \[R-8.8, X-18\].
  - ✅ I can extract an entry and show it to anyone to prove that an act took place, without revealing any content — the fingerprint it carries nevertheless makes it possible to confirm an exact guess \[R-8.7, R-9.3, X-19\].
  - ✅ I can prove that the date of an entry is indeed the one its author declared in it, and that it has not moved since \[R-8.10\].
  - ✅ I can prove that a content presented to me is exactly the one that was written at a given moment, by confronting it with the recorded fingerprint \[R-8.4, R-8.6\].
  - ❌ I cannot delete, modify or omit an entry \[R-8.8, R-8.12\].
  - ❌ I cannot prove that a date is exact: its author may have declared it false \[R-8.10, X-2\].
  - ❌ No reading appears in the journal \[R-8.11\].

### 12.8 Proving authenticity

  - ✅ I can prove to whoever holds a receipt that my Ethos is the continuation of the state frozen by that receipt \[R-9.6\].
  - ✅ I can ask a holder to countersign the state of my journal \[R-9.2\].
  - ❌ I cannot set aside the entry of another identity without setting aside everything that has been built upon it \[R-9.9, R-8.8\].
  - ❌ I cannot produce two different histories without leaving two incompatible receipts in the hands of their authors \[R-9.9\].
  - ❌ I cannot prove the authenticity of a portion of history that I wrote alone \[R-9.4, X-14\].
  - ❌ I cannot countersign: I hold no mandate \[R-9.2, R-6.8\].
  - ◆ A receipt freezes the run of the journal, never the validity of the acts it contains; a void entry from an identity other than mine renders binding just as another does \[R-9.10\].

### 12.9 Verifying

  - ✅ I can verify the entirety of the Ethos: author's marks, chains of mandates, coherence of the tree, continuity of the journal \[R-8.14\].
  - ✅ I can detect that a node has been modified, deleted or added without respecting the protocol: the Ethos is then corrupted, and the verification fails \[R-8.14\].
  - ✅ An entry recorded contrary to the rules does not corrupt the Ethos: it remains in the journal, void, I reject it, and everything that follows remains valid \[R-8.15\].
  - ✅ I can detect that an earlier version of the Ethos has been put back in place of the current one, if I hold a reference point kept outside the Ethos \[X-18\].
  - ❌ Without a reference point kept outside, I cannot detect a return backwards: the earlier state was coherent \[X-18\].

### 12.10 Limits

  - ❌ I cannot change owner identity, nor transmit the Ethos to another identity \[R-4.2\].
  - ◆ If my identity secret is lost, the Ethos is lost; if it is stolen, the thief is the owner \[X-13\].
  - ❌ I cannot prevent a holder from copying elsewhere what he has the right to read \[X-9\].
  - ◆ No access extinguishes itself: closing an access is always an act \[X-1, R-7.10\].
  - ◆ My other Ethoses have no link of state with this one: distinct Ethos identities, nothing is shared — but my public mark, readable on each one, links them in the eyes of anyone; for publicly separate vaults, I use distinct identities \[R-1.4, R-1.3\].
  - ◆ The methods that hold the Ethos are named by its identity, and age: nothing is promised against their weakening, and the only migration is a new Ethos \[R-1.13, X-22\].

## 13\. Delegate with read rights

### 13.1 With *browse* alone

  - ✅ I can see the index of P and of everything it contains, at every depth: the names, the sizes, the dates, the revisions \[R-5.2\].
  - ✅ I can see, for each node of P, whether it is private or public \[R-5.2, R-3.2\].
  - ✅ I can therefore designate any node of P \[R-5.5\].
  - ✅ I can read the content of the public nodes of P, like anyone \[R-3.2\].
  - ❌ I cannot read the content of any private file, not even that of P if P is a file \[R-5.2, R-5.1\].
  - ❌ Of the journal, I see only what anyone sees of it \[R-8.7\].

### 13.2 With *read* alone

  - ✅ If P is a file, I can read its content \[R-5.2\].
  - ✅ If P is a folder, I can read a file of P whose identifier I have learned through another channel \[R-5.5, R-5.8\].
  - ❌ I cannot link to P any name, any size, any date: I do not know what it contains. Of the nodes that anyone counts, I do not know which ones are in P \[R-5.8, R-10.1, R-10.3\] — except for the nestings of records that the correlation of metadata delivers to whoever observes the Ethos over time \[X-21\].
  - ❌ Without an identifier come from elsewhere, the right therefore remains void on a folder \[R-5.4, R-5.8\].
  - ◆ I can test whether an identifier learned elsewhere designates a node of P \[X-30\].

### 13.3 With *browse* and *read*

  - ✅ I can read the content of any file of P, at every depth \[R-5.2, R-5.5\].
  - ✅ I can copy what I read wherever I see fit \[X-9\].
  - ✅ I can read without anyone knowing that it is me: the holder of the medium sees a reading pass, never who reads \[R-10.5, X-24\].
  - ❌ I obtain no receipt from the mere fact of reading \[R-9.8\].
  - ❌ I cannot read an earlier version of a file of P \[R-2.8\].

### 13.4 In all cases

  - ✅ I can read the public nodes of the Ethos, wherever they are, and see their names \[R-3.2, R-3.6\].
  - ✅ I can see my own mandate — its scope, its rights — and each link of my chain, up to the owner, with its identity and its rights \[R-6.10, R-6.11\].
  - ✅ I can verify for myself that my chain is formally valid \[R-6.12\].
  - ✅ I can see all the mandates of the Ethos, like anyone: their issuers, their beneficiaries, their rights, and the identifier of their scope \[R-6.10\].
  - ✅ I can therefore see who holds a mandate on P, and, with *browse*, on each of the nodes contained in P \[R-6.10, R-5.5\].
  - ✅ I can verify that what I read was written under a mandate whose chain is valid: each node that I can read carries its author's mark, and the chain of the invoked mandate verifies on its own; that the node is indeed within the scope of that mandate is attested by the author, and is verified within the limit of what I see \[R-2.7, R-6.12, R-8.15\].
  - ✅ I can countersign the state of the journal, which earns me a receipt — countersigning is not a right: any holder can do it \[R-9.2, R-5.3\]. ◆ Nothing bounds their number: a flood of countersignatures swells the journal \[X-23].
  - ✅ I can count the nodes of the Ethos and see sizes, dates, revisions, like any third party \[R-10.1\].
  - ✅ My access lasts as long as my mandate is not revoked and its scope exists \[R-7.10, R-7.8\].
  - ❌ If P is private, I do not know its name: it is carried by its parent, outside my scope. I know P only by its identifier; the issuer can tell me through another channel what it is about, the protocol knows nothing of it \[R-3.6, R-2.6, R-5.6\].
  - ❌ I cannot attach a node situated outside my scope to a name, to a path, or to a parent \[R-10.3\].
  - ❌ I cannot see the name of P's parent, nor those of its sibling nodes, if they are private \[R-10.3, R-3.6\].
  - ❌ I cannot enumerate who has access to P from a node above: I know that whoever has access to P's parent has access to P, and I recognize the mandates that the public nestings place above me — starting with those of my own chain — nothing more \[X-8, R-6.11, R-7.8\].
  - ❌ I cannot know which node a mandate bearing outside my scope corresponds to \[R-10.3, R-2.6\].
  - ❌ I can read nothing outside P, save the public nodes \[R-6.1, R-3.2\].
  - ❌ I cannot survive the revocation of a link from which I descend \[R-7.2\].

### 13.5 What happens to me without my having any part in it

  - ✅ The owner writes elsewhere in the Ethos: I see no difference, and my access continues to work \[R-6.4, R-7.5\].
  - ✅ The owner publishes P: its content becomes readable by all, and I see it. My mandate is unchanged \[R-3.5, R-3.10\].
  - ✅ The owner closes P: I recover my access as holder, without receiving anything or doing anything — fully effective at the latest at the re-sealing, if revocations remained to be re-sealed \[R-3.9, X-27\].
  - ✅ The owner revokes someone else on P: my mandate does not descend from theirs, I keep my access without having done anything \[R-7.2, R-7.5\].
  - ✅ The owner deletes P: my mandate is deleted with it \[R-7.8\].
  - ✅ The owner empties P without deleting it: my mandate remains valid, and what will be created there afterwards will be accessible to me \[R-7.9\].
  - ✅ The owner revokes me: I no longer write, without notice, and everything that changes thereafter escapes me — completely at the latest at the re-sealing \[R-7.11, X-27\]; what I was able to read remains known to me \[R-7.3\].

## 14\. Delegate with write rights

Each section describes a right held **without** *browse*. Section 14.7 describes what *browse* adds to each of them.

### 14.1 With *add*

  - ✅ I can create a file or a folder in P: this requires designating no existing node \[R-5.9, R-5.8\].
  - ✅ I can create within a node I have myself created \[R-5.5\].
  - ✅ I propose a name; a uniqueness mark is appended to it, and none of my deposits fails \[R-2.1\].
  - ✅ I know the effective name of what I have created \[R-2.2\].
  - ❌ I cannot know what P contains, nor what the nodes found there are called \[R-5.8, R-10.3\].
  - ❌ I cannot learn through a creation that a name already exists \[R-2.1\].
  - ❌ I can create nothing if P is a file \[R-5.8\].
  - ❌ I cannot create beyond thirty-two levels of depth \[R-2.6\].

### 14.2 With *rename*

  - ✅ I can rename the files and folders I have myself created in P, and any node of P whose identifier I have learned through another channel \[R-2.3, R-5.5\].
  - ❌ I cannot rename any other node of P: I can designate none of them; the right remains void \[R-5.4, R-5.8\].
  - ❌ I cannot rename P itself: its name is borne by its parent \[R-5.9\].
  - ❌ I cannot impose a name already borne in the same folder: the renaming fails, and my tool can observe this before recording — I can indeed test a name without renaming \[R-2.4, X-12\].
  - ✅ Renaming a public node is an act like any other; the new name is public as long as the node is \[R-3.7\].

### 14.3 With *edit*

  - ✅ If P is a file, I can replace its content, in its entirety, without having read it \[R-5.2, R-5.9\].
  - ✅ I can replace the content of the files I have myself created \[R-5.5\].
  - ❌ I cannot modify only part of it: that requires *read* in addition \[R-5.9\].
  - ❌ If P is a folder, I can replace none of its files that I have not created, for lack of designating them \[R-5.4, R-5.8\].
  - ❌ I cannot rename: *edit* touches only a content \[R-5.1\].

### 14.4 With *delete*

  - ✅ I can empty P of all its content, in a single act, without designating anything nor reading any name or content — the anonymous shape of what I remove is known to me \[R-5.9, R-5.10, X-25\].
  - ✅ Emptying P deletes all the mandates that bore on what it contained; the mandates bearing on P itself remain valid \[R-7.8, R-7.9\].
  - ✅ I can delete the nodes I have myself created \[R-5.5\].
  - ❌ I cannot delete a specific node of P that I have not created, for lack of designating it \[R-5.4, R-5.8\].
  - ❌ I cannot delete P itself \[R-5.9\].

### 14.5 With *publish*

  - ✅ I can publish P itself, if it is private \[R-3.5, R-5.9\].
  - ✅ Once P is published, I can read all of its content like anyone, without holding *read* \[R-3.2, X-4\].
  - ✅ I can publish the nodes I have myself created \[R-5.5\].
  - ❌ I cannot publish a specific node of P that I have not created, for lack of designating it \[R-5.4, R-5.8\].
  - ❌ I cannot publish outside of P \[R-6.1\].
  - ❌ I cannot publish a node that is already public \[R-3.5\]. ✅ Publishing a folder that contained publication points absorbs them: the entry designates them \[R-3.11\].

### 14.6 With *close*

  - ✅ I can close P itself, if its publication point is P \[R-3.8, R-5.9\].
  - ✅ All the holders concerned regain their access without receiving anything from me — fully effective at the latest upon the re-sealing, if revocations remained to be re-sealed \[R-3.9, X-27\].
  - ❌ I cannot close a node whose publication point is above P \[R-3.8, R-6.1\].
  - ❌ I cannot close a node whose quality as a point has been absorbed by a higher publication \[R-3.11\].
  - ❌ I cannot close a specific node of P that I have not created, for lack of designating it \[R-5.4, R-5.8\].

### 14.7 What *browse* adds

*browse* makes known to me the nodes contained in P \[R-5.5\]. I can therefore designate them, and exercise on each of them the rights I hold:



  - ✅ With *add*, create in any folder of P; with *rename*, rename any node of P without having read its content; with *edit*, replace in its entirety any file of P — and modify only part of it with *read* in addition; with *delete*, delete any node of P; with *publish* or *close*, act on any node of P; with *delegate*, delegate any node of P \[R-5.5, R-5.9\].
  - ❌ The prohibitions that do not stem from designation remain: renaming P, deleting P, acting outside of P \[R-5.9, R-6.1\].
  - ❌ *browse* gives me no content of a private file \[R-5.2, R-5.1\].

### 14.8 In all cases

  - ✅ Every write, whatever the right exercised, is recorded in the journal, attributed to my identity, attached to my mandate, and earns me a receipt. I have nothing to request and nothing to activate \[R-8.2, R-8.3, R-9.1\].
  - ✅ What I write bears my author's mark, and the entry bears the fingerprint of the state after the act \[R-2.7, R-8.4\].
  - ✅ Recording an entry does not require being able to read the journal \[R-8.9\].
  - ✅ My entry bears names and designations that I need not know and cannot always read: it bears them without showing them to me \[R-8.3, R-8.16\].
  - ✅ I write without the owner being present: my write is complete, valid and verifiable on its own \[R-6.3, R-8.14\].
  - ✅ What I create is private or public according to the place where I create it \[R-3.3\], and accessible to all those who have access to P, including the owner \[R-6.4, R-1.11\].
  - ✅ A node I have created is designatable by me: all my other rights apply to it \[R-5.5\].
  - ✅ An identifier learned through another channel allows me to exercise my rights on that node, even without *browse* — by attesting a coverage that my rights may allow me to test \[X-30] : outside my scope, my entry would be void \[R-5.5, R-8.15].
  - ❌ I cannot act outside of P \[R-6.1\].
  - ❌ I cannot move a node \[R-2.5\].
  - ❌ I can no longer do anything as soon as my revocation is recorded; my earlier writes remain valid \[R-7.3\].
  - ❌ I cannot write without recording an entry in the journal \[R-8.2\].

## 15\. Delegate with the right to delegate

A mandate including *delegate*, on the node P.



  - ✅ I can delegate P, with everything it contains; with *browse*, I can delegate any node of P \[R-6.1, R-5.5\].
  - ✅ I can issue as many mandates as I want, to as many identities as I want — including my own, never that of the owner \[R-6.8\].
  - ✅ To issue, I invoke a single link: I can give any subset of the rights of that link — never the union of my mandates —, and give or refuse *delegate* \[R-6.7, R-8.3, R-5.7\].
  - ✅ I can revoke any mandate that descends from mine, whether it was issued by me or by one of my sub-delegates \[R-7.1, R-7.2\].
  - ✅ Revoking a mandate at depth does not bring down the other branches: the holders whose mandate does not descend from the revoked mandate keep their access, without receiving anything \[R-7.2, R-7.5\].
  - ✅ I see all the mandates of the Ethos, like anyone \[R-6.10\]. Those I can situate within P: those that descend from a mandate on P, and those whose scope is a node I know \[R-6.11, R-5.5\].
  - ❌ Without *browse*, I can delegate only P itself, a node I have created there, or a node whose identifier I have learned elsewhere \[R-5.8, R-5.5\].
  - ❌ I cannot give a right I do not have, nor a scope broader than P \[R-6.7\].
  - ❌ I cannot issue a mandate if my chain already counts thirty-two links \[R-6.7\].
  - ❌ I cannot revoke a mandate that does not descend from mine, even if it bears on a node of my scope \[R-7.1\].
  - ❌ I cannot prevent the owner, nor a link from which I descend, from revoking a mandate I have issued \[R-7.1\].
  - ◆ To delegate a node known by its identifier alone is to attest a nesting I have not seen — my rights may allow me to test it \[X-30] : if I delegate without testing an identifier outside my scope, my issuance — and everything that descends from it — is a void entry \[R-5.5, R-6.12, R-8.15].
  - ✅ After a revocation I have been able to make, I can re-seal its scope: the exclusion going forward becomes complete \[R-7.11, X-27\].



When my own mandate falls:



  - ✅ All the mandates that descend from mine fall with it, as soon as it is recorded \[R-7.2, R-7.3\].
  - ✅ My sub-delegates are revoked exactly as I am, with the same reach \[R-7.3\].
  - ✅ What we have written remains in the Ethos, valid, attributed to our identities and traced in the journal \[R-7.3, R-8.12\].

## 16\. Auditor

A mandate including *audit*, on the node P.

### 16.1 With *audit* alone

  - ✅ I can read all the entries of the journal that concern P and what it contains \[R-5.2, R-8.6\].
  - ✅ I see, for each entry: the author, the action, the mandate invoked, the identifiers of the nodes touched within P, the fingerprints borne, its place in the order, the declared date \[R-8.7\].
  - ✅ I can verify that these entries have not been truncated, added to, or reordered \[R-8.8\].
  - ✅ I can verify that a node of P indeed corresponds to the last valid entry bearing a fingerprint of it — a folder being concerned by everything that modifies its index \[R-8.6, R-8.15\].
  - ✅ I can reject a node of P whose entry is missing \[R-8.13\].
  - ✅ I can verify that a write recorded in P was covered by a formally valid mandate — the coverage beyond what my entries show me is attested \[R-8.7, R-6.12, R-6.11\].
  - ✅ I can confirm that a content presented to me is indeed the one that was written, by comparing it to its fingerprint \[R-8.4, R-8.6\].
  - ✅ I can extract an entry and show it to anyone \[R-8.7, R-9.3\].
  - ✅ If my mandate bears on the root, I read the whole journal \[R-5.9\].
  - ✅ I can countersign the state of the journal \[R-9.2\].
  - ❌ I see no name: the nodes are known to me only by their identifier \[R-8.7\].
  - ❌ I cannot situate P itself in the tree, nor learn anything of the nestings outside of P — apart from the anonymous containment that public correlation delivers to anyone \[R-10.3, X-21\]. ✅ Inside P, the creation and emptying entries deliver to me the nesting of the identifiers, step by step \[R-8.4, R-8.7\].
  - ❌ I can read no content of a file \[R-5.1, R-5.2\].
  - ❌ Of entries that concern only nodes outside of P, I see only what anyone sees of them \[R-8.7\].

### 16.2 With *audit* and *browse*

  - ✅ I see the names borne by each entry, as they stood at the moment of the act — including the old and new names of the renamings \[R-8.7, R-8.4\].
  - ✅ I can attach each entry to its place in the tree \[R-5.2\].
  - ✅ I can read the complete history of a deleted node: its name at each act, what happened to it, and who deleted it \[R-8.4, R-8.12, R-8.7\].
  - ✅ I can follow it even if it disappeared along with a whole folder: the entry of its creation attaches it to its folder, and the entry that carried away that folder dates its end — nothing enters a folder nor leaves it outside the journal, since nothing moves \[R-8.4, R-8.6, R-2.5\].
  - ❌ I still can read no content of a file \[R-5.1\].
  - ❌ I cannot recover the content of a deleted file \[R-2.8, X-11\].

### 16.3 In all cases

  - ❌ I can create, rename, modify, delete, publish, close or delegate nothing \[R-5.1\].
  - ❌ I cannot know who has read what \[R-10.5\].
  - ❌ I cannot establish the real date of an act, only the one its author declared, and its place in the order \[R-8.10, X-2\].
## 17\. Former holder

  - ✅ I keep what I extracted before the revocation; what I was able to learn while I had the right to is deemed learned \[R-7.3, X-9, R-1.12\].
  - ✅ Everything that is modified or created since my departure is closed to me. What I could read and which has not changed, I may still hold: my departure takes back from me nothing of what I knew \[R-7.3\].
  - ✅ I can read the public nodes, like anyone, including those of my former scope \[R-7.6, R-3.2\].
  - ✅ My past acts remain in the Ethos, valid, attributed and traced \[R-7.3, R-8.12\].
  - ✅ I keep my receipts: the revocation changes nothing of their force \[R-9.7\].
  - ✅ As a former auditor, I keep the reading of the entries prior to my revocation; of later entries, I see only what a third party sees of them \[R-7.7\].
  - ❌ I can no longer write anything, as soon as the revocation is recorded \[R-7.3\].
  - ❌ Of what has happened since my revocation, I see only what anyone sees of it \[R-7.3, R-8.7, R-10.1\].
  - ◆ As long as my former scope is not re-sealed, a new write may still be readable to me; the re-sealing closes this interstice \[R-7.11, X-27\].
  - ◆ I keep forever the tracking of the records whose identifier I have known: their public metadata and their activity — never, as long as they remain private, their contents nor their new names \[X-29, R-10.2, X-21\].
  - ❌ I cannot make binding a receipt that nothing else has followed, or that only entries of the owner have followed \[R-9.4\].
  - ◆ Nothing materially prevents me from recording an entry: it is void, but it remains, and it freezes the journal like a receipt of passage \[R-8.15, R-9.10, X-23\].

## 18\. Witness

  - ✅ I can require the proof that the Ethos presented to me is the continuation of the state frozen by my receipt, and verify it myself, without access \[R-9.6\].
  - ✅ I can keep my receipt indefinitely, transmit it or publish it: it reveals no content, no name, no structure, no node size — it reveals the Ethos concerned, the frozen instant, my identity and my declared date \[R-9.3\].
  - ✅ I can compare my receipt with that of another witness freezing the same instant, outside the presence of the Ethos: if they differ, there is fraud — equivocation of the medium or a lying receipt — and anyone who observes it, without access; in the presence of the Ethos, the proof of continuation establishes only which of the two receipts the presented state continues \[R-9.5, R-9.6\].
  - ✅ In the presence of the Ethos, I can establish the compatibility of my receipt with any older or more recent receipt, by a proof of continuation that each verifies alone \[R-9.6\].
  - ✅ I keep this power after having lost all access to the Ethos \[R-9.3, R-9.7\].
  - ✅ My receipt freezes the run of the journal, never the validity of the acts it contains \[R-9.10\].
  - ❌ I cannot detect alone that a different history is being presented to someone else: two receipts must be compared \[R-9.5\].
  - ❌ Outside the presence of the Ethos, and without a proof of continuation, I can establish nothing between two receipts of different instants \[R-9.5, R-9.6\].
  - ❌ I cannot make binding a receipt that nothing else has followed, or that only entries of the owner have followed \[R-9.4\].
  - ❌ I cannot establish the real date of my receipt, only the one its author declared in it and its place in the order \[R-8.10, X-2\].
  - ❌ I can learn nothing of the content of the Ethos from a receipt \[R-9.3\].
  - ◆ Nothing materially prevents me from recording an entry: it is void, but it remains, and it freezes the journal like a receipt of passage \[R-8.15, R-9.10, X-23\].

## 19\. Third party

Anyone who holds neither the ownership of the Ethos, nor any mandate in force — including whoever holds the medium \[R-1.10\].



  - ✅ I can read the content of all the public nodes, see their names, and browse the index of the public folders, with any tool \[R-3.2, R-3.6\].
  - ✅ I can verify that a public node is indeed what its author wrote — of the state presented to me: that it is the latest is confronted with the journal, with *audit* or a reference point —, by whom, and under which mandate: the node carries its author's mark, and the chain verifies itself alone, formally \[R-2.7, R-8.6, X-18, R-6.12\].
  - ✅ I can verify that the journal is continuous: no entry removed, inserted nor reordered — except the removal of the last entries, undetectable without an external reference point \[R-8.8, X-18\].
  - ✅ I can see, for each entry, its place, its author — attested in an unforgeable way — and its declared date \[R-8.7\].
  - ✅ I can read in full the public entries — mandates issued, revocations, countersignatures — and the public mentions of mandates carried away by a deletion \[R-8.5, R-7.8\].
  - ✅ I can see all the mandates, in full, and verify that the chains are formally valid up to the owner — their nestings are attested, not verifiable by me \[R-6.10, R-6.12, R-6.11\].
  - ✅ I can see the nesting of the scopes that the chains reveal \[R-6.11\].
  - ✅ I can read the identity of the Ethos and the public mark of its owner — two distinct identities, the first fixed at creation \[R-1.2, R-1.3\].
  - ✅ I can verify a receipt presented to me, verify a proof of continuation, and observe that two receipts of one same instant are incompatible \[R-9.3, R-9.5, R-9.6\].
  - ✅ I can count the nodes of the Ethos and see sizes, dates, revisions, states \[R-10.1\].
  - ✅ I can verify everything I see: the corrupted Ethos fails my verification \[R-8.14\].
  - ❌ I can read no private content, nor see the name of any private node \[R-10.1, R-10.2\].
  - ❌ I cannot reconstitute the private tree structure: nodes without knowing which ones, scope identifiers without knowing where \[R-10.1, R-10.3\]. ◆ I do learn, however, as the writes go by, the anonymous containment of the records that change \[X-21\].
  - ❌ I cannot know what a mandate corresponds to: I observe that a public mark has rights over an identifier, not over what \[R-10.3, R-2.6\].
  - ❌ Of a non-public entry I can read neither the identifiers, nor the names, nor the fingerprints, nor the mandate invoked \[R-8.7, X-20\]. ◆ I can however correlate each entry to the records it moves and to the type of act, through the public metadata \[X-21\].
  - ❌ I cannot reject a node whose entry is missing: that requires *audit* on that node — except the record appeared with no entry at all before my eyes: a corrupted artifact, observable without access \[R-8.13, R-8.14, X-21\].
  - ❌ Outside the public nodes, I cannot verify that a write was covered by a mandate: that requires *audit* \[R-8.7\].
  - ◆ As holder of the medium, I see the accesses that reach me pass by — reads and writes: never the identity that reads, and the Ethos keeps no trace of the reads; the burst of writes lets me circumscribe the extent of a subtree published, closed or re-sealed \[X-24, R-7.11, R-10.5\].
  - ◆ Nothing materially prevents me from recording an entry: it is void, but it remains, and it freezes the journal like a receipt of passage \[R-8.15, R-9.10, X-23\].

## 20\. Cross-cutting cases

  - ✅ Two mandates on the same node for the same identity: the rights add up — since each act invokes only a single mandate, an operation with two rights requires them from the same link \[R-6.4, R-8.3, R-5.9\].
  - ✅ A mandate on a node and another, broader, on one of its parents: the identity accumulates both \[R-6.4\].
  - ✅ An identity that loses a mandate but keeps another covering the same node retains access through this second mandate \[R-7.5\].
  - ✅ A node published then closed: the mandates issued during the publication, reading included, take their full effect at the closing — fully effective at the latest at the re-sealing, if revocations remained to be re-sealed \[R-6.9, R-3.10, X-27\].
  - ✅ One same identity used from several machines is a single identity, and the journal does not distinguish them \[R-1.9\].
  - ✅ An entry recorded contrary to the rules remains in the journal, void; the entries that follow it remain valid, and the Ethos is not corrupted \[R-8.15, R-8.14\].
  - ✅ A mandate revoked, then its scope re-sealed: nothing that is written there afterwards is within the reach of the revoked party \[R-7.11, X-27\].
  - ❌ Two recordings cannot be simultaneous on a medium that serializes, from whomever they come: the second waits or fails \[R-1.7\]. ◆ A hostile medium that maintains two runs leaves an equivocation, detectable by confrontation of receipts \[R-1.7, X-17, R-9.5\].
# Annex A — Roles × acts × states matrix

This matrix makes coverage checkable: each cell cites the rule (R-x.y) or the assumed limit (X-n) that covers it. A cell without qualification holds for all states.

  

**Roles.** *Owner*; *Holder* — according to the rights carried by its mandate on P (read, write, delegator, auditor); *Former holder*; *Witness* — holder of a receipt, with no other access; *Third party* — including the holder of the medium (R-1.10).

  

**States.** private / public (R-3.1) · within / outside scope (R-6.1) · root (R-1.8) · before / during / after mandate (R-7.3). The cells note the distinctions where they exist.

  

|  |  |  |  |  |  |
| :-: | :-: | :-: | :-: | :-: | :-: |
| \*\*Act\*\* | \*\*Owner\*\* | \*\*Holder (according to its rights)\*\* | \*\*Former holder\*\* | \*\*Witness\*\* | \*\*Third party\*\* |
| \*\*Create\*\* | ✅ everywhere R-1.11; ❌ exact name R-2.1; ❌ beyond the depth bound R-2.6 | ✅ \*add\*, in P or a designated folder R-5.9, R-5.5, R-5.8; ❌ outside P R-6.1; ❌ in a file R-5.8; ❌ beyond the bound R-2.6; inherited visibility R-3.3 | ❌ R-7.3 | ❌ no right R-5.9 | ❌ no right R-5.9, R-5.4 |
| \*\*Rename\*\* | ✅ everything except the root R-1.11, R-1.8; ❌ collision R-2.4 | ✅ \*rename\* + designation R-2.3, R-5.5; ❌ P itself R-5.9; ❌ collision R-2.4, X-12 | ❌ R-7.3 | ❌ R-5.9 | ❌ R-5.9 |
| \*\*Edit\*\* | ✅ any file R-1.11 | ✅ \*edit\*, in full R-5.9; a part: + \*read\* R-5.9; without \*browse\*: P, creations, known identifiers R-5.8 | ❌ R-7.3 | ❌ R-5.9 | ❌ R-5.9 |
| \*\*Delete\*\* | ✅ everything except the root R-1.11, R-1.8; mandates carried away R-7.8 | ✅ \*delete\* + designation R-5.9, R-5.5; ❌ P itself R-5.9 | ❌ R-7.3 | ❌ R-5.9 | ❌ R-5.9 |
| \*\*Empty\*\* | ✅ any folder, root included R-5.9, R-1.8 | ✅ \*delete\* on the folder, without designating or seeing R-5.9, R-5.10, X-5; mandates: R-7.8, R-7.9 | ❌ R-7.3 | ❌ R-5.9 | ❌ R-5.9 |
| \*\*Read\*\* | ✅ everything R-1.11; ❌ earlier versions R-2.8 | ✅ \*read\* + designation R-5.5, R-5.8; public: like anyone R-3.2 | before: acquired if unchanged R-7.3, X-9; after: ❌ R-7.3; public: ✅ R-7.6 | public only R-3.2 | public only R-3.2; private: ❌ R-10.1 |
| \*\*Browse\*\* | ✅ everything R-1.11 | ✅ \*browse\*: names, sizes, dates, revisions, states, identifiers R-5.2; outside scope: R-10.3 | before: index acquired if unchanged R-7.3, X-9; after: ❌ R-7.3; public indexes: ✅ R-3.6 | public indexes R-3.6; counting R-10.1 | public indexes R-3.6; counting R-10.1 |
| \*\*Audit\*\* | ✅ the whole journal R-1.11 | ✅ \*audit\* on the entries concerning P R-8.6, R-8.7; names: + \*browse\* R-8.7; root → everything R-5.9 | earlier entries: ✅; later ones: like a third party R-7.7 | public portion R-8.7, R-8.5, R-7.8 | public portion R-8.7, R-8.5, R-7.8; ❌ the rest X-20; correlation X-21 |
| \*\*Publish\*\* | ✅ any private node R-1.11, R-3.5; interior points absorbed R-3.11 | ✅ \*publish\*: P, creations, designated ones R-3.5, R-5.8; ❌ already public R-3.5; ❌ outside P R-6.1; absorption R-3.11 | ❌ R-7.3 | ❌ R-5.9 | ❌ R-5.9 |
| \*\*Close\*\* | ✅ at the publication point R-3.8 | ✅ \*close\*, if the point is P, a creation or a designated one R-3.8, R-5.8; ❌ point above P R-3.8; ❌ absorbed node R-3.11 | ❌ R-7.3 | ❌ R-5.9 | ❌ R-5.9 |
| \*\*Delegate\*\* | ✅ to any identity except its own R-6.8; public node included R-6.9 | ✅ \*delegate\*: subset of the rights of the invoked link, on P or a designated one R-6.7, R-5.8; ❌ beyond thirty-two links R-6.7; to itself: ✅ R-6.8; ❌ to the owner R-6.8 | ❌ R-7.3 | ❌ R-6.7 | ❌ R-6.7 |
| \*\*Revoke\*\* | ✅ any mandate R-7.1 | ✅ any mandate descending from its own R-7.1, R-7.2; ❌ the others R-7.1 | ❌ R-7.3 | ❌ R-7.1 | ❌ R-7.1 |
| \*\*Re-seal\*\* | ✅ any revoked scope R-7.11 | ✅ if it could revoke the mandate in question R-7.11, R-7.1; ❌ otherwise R-7.11 | ❌ R-7.3 | ❌ R-7.11 | ❌ R-7.11 |
| \*\*Countersign\*\* | ❌ R-9.2; ◆ a void entry from the owner freezes the journal without making anything binding R-8.15, R-9.10, X-14 | ✅ whatever its rights R-9.2 | ❌ is no longer a holder R-9.2, R-7.3; ◆ a void entry freezes the journal all the same R-8.15, R-9.10, X-23 | ❌ is not a holder R-9.2; ◆ same qualification R-9.10, X-23 | ❌ R-9.2; ◆ same qualification R-9.10, X-23 |
| \*\*Verify, compare receipts\*\* | ✅ R-8.14, R-9.5, R-9.6 | ✅ R-8.14, R-6.12, R-9.5 | ✅ R-8.14, R-9.5, R-9.7 | ✅ R-9.3, R-9.5, R-9.6 | ✅ R-8.14, R-9.3, R-9.5 |
| \*\*Create an Ethos\*\* | ✅ R-1.1; replaying its creation: X-15 | ✅ with its own identity R-1.1, R-1.5; ❌ at the identity of an existing Ethos R-1.6 | ✅ R-1.1; ❌ same R-1.6 | ✅ R-1.1; ❌ same R-1.6 | ✅ R-1.1; ❌ at the identity of an existing Ethos R-1.6 |

  

**Closure counter-tests.** Reads appear in no journal, for no role (R-10.5). The before / during / after mandate states are carried by R-7.3, R-7.5, R-7.6 and R-7.7 — and, for the effectiveness of exclusion as to the future, by R-7.11 and X-27; the "published then closed" case by R-6.9, R-3.10 and R-3.12; acts on the root by R-1.8 and R-8.3; what every role sees at all times by R-10.1 to R-10.5; the public end of mandates by R-8.5 and R-7.8; the fate of void entries by R-8.15 and R-9.10; what is promised to no one by X-1 to X-30.

  
