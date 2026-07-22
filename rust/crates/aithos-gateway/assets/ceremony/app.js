import init, {
  build_ceremony_challenge,
  DelegateSigner,
  verify_mandate_chain,
} from "./aithos_wasm.js";

const main = document.querySelector("main[data-ceremony]");
const status = document.querySelector("#ceremony-status");
const keystoreInput = document.querySelector("#keystore");
const passphraseInput = document.querySelector("#passphrase");
const unlockButton = document.querySelector("#unlock");
const parentPanel = document.querySelector("#parent-panel");
const parentSelect = document.querySelector("#parent");
const reviewPanel = document.querySelector("#review-panel");
const review = document.querySelector("#review");
const authorizeButton = document.querySelector("#authorize");
const cancelButton = document.querySelector("#cancel");

let signer = null;
let preparation = null;
let selectedLeaf = null;
let selectedGrant = null;
let selectedChallenge = null;
let completed = false;

function setStatus(message, kind = "") {
  status.textContent = message;
  status.dataset.kind = kind;
}

function decodeBase64Url(value, label) {
  if (typeof value !== "string" || !/^[A-Za-z0-9_-]+$/.test(value)) {
    throw new Error(`${label} is not valid base64url`);
  }
  const padded = value.replace(/-/g, "+").replace(/_/g, "/")
    + "=".repeat((4 - value.length % 4) % 4);
  const decoded = atob(padded);
  return Uint8Array.from(decoded, (character) => character.charCodeAt(0));
}

async function decryptKeystore(document, passphrase) {
  if (!document || document["aithos-keystore"] !== "1.0.0") {
    throw new Error("Unsupported Aithos keystore profile");
  }
  const { kdf, cipher, public_key: publicKey } = document;
  if (kdf?.name !== "PBKDF2" || kdf.hash !== "SHA-256"
      || !Number.isSafeInteger(kdf.iterations)
      || kdf.iterations < 600000 || kdf.iterations > 2000000) {
    throw new Error("The keystore KDF parameters are not accepted");
  }
  if (cipher?.name !== "AES-GCM" || typeof publicKey !== "string") {
    throw new Error("The keystore cipher or public key is malformed");
  }
  const salt = decodeBase64Url(kdf.salt, "KDF salt");
  const iv = decodeBase64Url(cipher.iv, "cipher IV");
  const ciphertext = decodeBase64Url(cipher.ciphertext, "ciphertext");
  if (salt.length < 16 || iv.length !== 12 || ciphertext.length !== 48) {
    salt.fill(0);
    iv.fill(0);
    ciphertext.fill(0);
    throw new Error("The keystore encryption parameters are malformed");
  }
  const passphraseBytes = new TextEncoder().encode(passphrase);
  try {
    const material = await crypto.subtle.importKey(
      "raw",
      passphraseBytes,
      "PBKDF2",
      false,
      ["deriveKey"],
    );
    const key = await crypto.subtle.deriveKey(
      { name: "PBKDF2", hash: "SHA-256", salt, iterations: kdf.iterations },
      material,
      { name: "AES-GCM", length: 256 },
      false,
      ["decrypt"],
    );
    const plaintext = await crypto.subtle.decrypt({ name: "AES-GCM", iv }, key, ciphertext);
    const seed = new Uint8Array(plaintext);
    if (seed.length !== 32) {
      seed.fill(0);
      throw new Error("The decrypted signer seed has the wrong length");
    }
    return { seed, publicKey };
  } catch (error) {
    throw new Error("The keystore could not be unlocked", { cause: error });
  } finally {
    passphraseBytes.fill(0);
    salt.fill(0);
    iv.fill(0);
    ciphertext.fill(0);
  }
}

async function postJson(path, body, accept = "application/json") {
  const response = await fetch(path, {
    method: "POST",
    credentials: "same-origin",
    cache: "no-store",
    headers: { "content-type": "application/json", accept },
    body: JSON.stringify(body),
  });
  const answer = response.status === 204 ? null : await response.json().catch(() => null);
  if (!response.ok) {
    throw new Error(answer?.error_description || "The gateway refused the ceremony");
  }
  return answer;
}

function destroySigner() {
  if (signer) {
    signer.free();
    signer = null;
  }
  passphraseInput.value = "";
  keystoreInput.value = "";
}

function randomHex(length) {
  const bytes = crypto.getRandomValues(new Uint8Array(length));
  const value = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  bytes.fill(0);
  return value;
}

function randomUlid() {
  const alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  let value = 0n;
  for (const byte of bytes) value = (value << 8n) | BigInt(byte);
  bytes.fill(0);
  let encoded = "";
  for (let index = 0; index < 26; index += 1) {
    encoded = alphabet[Number(value & 31n)] + encoded;
    value >>= 5n;
  }
  return encoded;
}

function canonicalEightHourCeiling(at) {
  return new Date(Date.parse(at) + 8 * 60 * 60 * 1000).toISOString().replace(".000Z", "Z");
}

function laterInstant(left, right) {
  return Date.parse(left) >= Date.parse(right) ? left : right;
}

function earlierInstant(left, right) {
  return Date.parse(left) <= Date.parse(right) ? left : right;
}

function addReview(label, value, preformatted = false) {
  const term = document.createElement("dt");
  term.textContent = label;
  const description = document.createElement("dd");
  const content = document.createElement(preformatted ? "pre" : "code");
  content.textContent = value;
  description.append(content);
  review.append(term, description);
}

function renderReview(parent, leaf, challengeEnvelope) {
  review.replaceChildren();
  const bindings = preparation.bindings;
  const seconds = Math.max(0, Math.floor(
    (Date.parse(leaf.not_after) - Date.parse(leaf.not_before)) / 1000,
  ));
  addReview("Gateway host", location.host);
  addReview("OAuth client", bindings.client_id);
  addReview("Resource", bindings.resource);
  addReview("Ethos / context", parent.context);
  addReview("Mandate chain", parent.chain.map((mandate) => mandate.id).join(" → "));
  addReview("Session perimeter", JSON.stringify(leaf.perimeter, null, 2), true);
  addReview("Constraints and obligations", JSON.stringify(leaf.constraints, null, 2), true);
  addReview("Session lifetime", `${seconds} seconds · until ${leaf.not_after}`);
  addReview("Gateway signing key", bindings.gateway_pub);
  addReview("Gateway KEX key", bindings.gateway_kex_pub);
  addReview("Session public key", bindings.session_pub);
  addReview("Ceremony nonce", bindings.nonce);
  addReview("WYSIWYS digest", challengeEnvelope.digest);
}

async function buildSelection() {
  if (!signer || !preparation) return;
  const parent = preparation.eligible_parents[Number(parentSelect.value)];
  if (!parent) return;
  const verifiedAt = preparation.verified_at;
  verify_mandate_chain(
    JSON.stringify(parent.chain),
    JSON.stringify(parent.did),
    verifiedAt,
    JSON.stringify(parent.revocations),
  );
  const bindings = preparation.bindings;
  const request = {
    id: `mandate_${randomUlid()}`,
    subject: parent.subject,
    grantee_id: `urn:aithos:agent:mcp-session-${randomHex(8)}`,
    grantee_label: "MCP delegated session",
    gateway_pub: bindings.gateway_pub,
    gateway_kex_pub: bindings.gateway_kex_pub,
    session_pub: bindings.session_pub,
    perimeter: parent.session_perimeter,
    constraints: JSON.parse(JSON.stringify(parent.constraints)),
    not_before: laterInstant(parent.not_before, verifiedAt),
    not_after: earlierInstant(parent.not_after, canonicalEightHourCeiling(verifiedAt)),
    issued_at: verifiedAt,
    nonce: randomHex(16),
  };
  const leafJson = signer.build_session_submandate(
    JSON.stringify(parent.chain.at(-1)),
    JSON.stringify(request),
  );
  const leaf = JSON.parse(leafJson);
  verify_mandate_chain(
    JSON.stringify([...parent.chain, leaf]),
    JSON.stringify(parent.did),
    verifiedAt,
    JSON.stringify(parent.revocations),
  );
  const preparedGrant = await postJson("/ceremony/prepare-grant", {
    transaction_id: bindings.transaction_id,
    delegate_pub: bindings.delegate_pub,
    context: parent.context,
    parent_id: parent.parent_id,
    leaf,
  });
  if (preparedGrant?.v !== 1 || !preparedGrant.grant) {
    throw new Error("The gateway returned a malformed delegated grant");
  }
  const signedGrant = JSON.parse(
    signer.sign_delegated_grant(JSON.stringify(preparedGrant.grant)),
  );
  const challengeEnvelope = JSON.parse(build_ceremony_challenge(
    JSON.stringify(bindings),
    parent.context,
    parent.parent_id,
    leafJson,
    JSON.stringify(signedGrant),
  ));
  selectedLeaf = leaf;
  selectedGrant = signedGrant;
  selectedChallenge = challengeEnvelope.challenge;
  renderReview(parent, leaf, challengeEnvelope);
  reviewPanel.classList.remove("hidden");
  setStatus("The chain and strict attenuation were verified locally. Review before signing.", "ok");
}

unlockButton.addEventListener("click", async () => {
  unlockButton.disabled = true;
  preparation = null;
  selectedLeaf = null;
  selectedGrant = null;
  selectedChallenge = null;
  setStatus("Unlocking and verifying locally…");
  try {
    const file = keystoreInput.files?.[0];
    if (!file || !passphraseInput.value) throw new Error("Choose a keystore and enter its passphrase");
    const keystoreDocument = JSON.parse(await file.text());
    const { seed, publicKey } = await decryptKeystore(keystoreDocument, passphraseInput.value);
    passphraseInput.value = "";
    try {
      signer = new DelegateSigner(seed);
    } finally {
      seed.fill(0);
    }
    const delegatePub = signer.public_key();
    if (delegatePub !== publicKey) {
      throw new Error("The keystore public key does not match its encrypted signer");
    }
    preparation = await postJson("/ceremony/prepare", {
      transaction_id: main.dataset.ceremony,
      delegate_pub: delegatePub,
    });
    if (preparation.v !== 1 || !Array.isArray(preparation.eligible_parents)) {
      throw new Error("The gateway returned malformed ceremony data");
    }
    if (preparation.eligible_parents.length === 0) {
      throw new Error("No fresh mandate with session-issuing authority is eligible for this signer");
    }
    parentSelect.replaceChildren();
    preparation.eligible_parents.forEach((parent, index) => {
      const option = document.createElement("option");
      option.value = String(index);
      option.textContent = `${parent.context} · ${parent.parent_id} · expires ${parent.not_after}`;
      parentSelect.append(option);
    });
    parentPanel.classList.remove("hidden");
    await buildSelection();
  } catch (error) {
    if (preparation) {
      await postJson("/ceremony/cancel", { transaction_id: main.dataset.ceremony }).catch(() => null);
      completed = true;
    }
    destroySigner();
    setStatus(error instanceof Error ? error.message : "The signer could not be unlocked", "error");
  } finally {
    unlockButton.disabled = false;
  }
});

parentSelect.addEventListener("change", async () => {
  try {
    selectedLeaf = null;
    selectedGrant = null;
    selectedChallenge = null;
    reviewPanel.classList.add("hidden");
    setStatus("Preparing the exact delegated grant…");
    await buildSelection();
  } catch (error) {
    selectedLeaf = null;
    selectedGrant = null;
    selectedChallenge = null;
    reviewPanel.classList.add("hidden");
    setStatus(error instanceof Error ? error.message : "The selected chain was refused", "error");
  }
});

authorizeButton.addEventListener("click", async () => {
  if (!signer || !selectedLeaf || !selectedGrant || !selectedChallenge || !preparation) return;
  authorizeButton.disabled = true;
  parentSelect.disabled = true;
  setStatus("Signing inside WASM and completing the one-shot ceremony…");
  try {
    const parent = preparation.eligible_parents[Number(parentSelect.value)];
    const proof = JSON.parse(signer.sign_ceremony_challenge(JSON.stringify(selectedChallenge)));
    const answer = await postJson("/ceremony/complete", {
      transaction_id: preparation.bindings.transaction_id,
      context: parent.context,
      parent_id: parent.parent_id,
      leaf: selectedLeaf,
      grant: selectedGrant,
      proof,
    });
    if (typeof answer?.redirect_to !== "string") {
      throw new Error("The gateway did not return the OAuth callback");
    }
    completed = true;
    destroySigner();
    setStatus("Authorized. Returning to the OAuth client…", "ok");
    location.assign(answer.redirect_to);
  } catch (error) {
    destroySigner();
    setStatus(error instanceof Error ? error.message : "The ceremony was refused", "error");
  } finally {
    authorizeButton.disabled = false;
  }
});

cancelButton.addEventListener("click", async () => {
  cancelButton.disabled = true;
  try {
    await postJson("/ceremony/cancel", { transaction_id: main.dataset.ceremony });
  } catch (_) {
    // Cancellation is idempotent; local key destruction still wins.
  }
  completed = true;
  destroySigner();
  parentPanel.classList.add("hidden");
  reviewPanel.classList.add("hidden");
  setStatus("Canceled. No authorization code or session was created.", "ok");
});

addEventListener("pagehide", () => {
  destroySigner();
  if (!completed) {
    const body = new Blob(
      [JSON.stringify({ transaction_id: main.dataset.ceremony })],
      { type: "application/json" },
    );
    navigator.sendBeacon("/ceremony/cancel", body);
  }
});

try {
  await init({ module_or_path: new URL("./aithos_wasm_bg.wasm", import.meta.url) });
  setStatus("Local verifier ready. Choose your encrypted keystore.", "ok");
} catch (_) {
  unlockButton.disabled = true;
  setStatus("The local verifier could not be loaded. Nothing was authorized.", "error");
}
