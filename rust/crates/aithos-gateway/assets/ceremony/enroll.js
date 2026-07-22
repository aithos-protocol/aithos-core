import init, { DelegateSigner } from "./aithos_wasm.js";

const passphraseInput = document.querySelector("#passphrase");
const confirmationInput = document.querySelector("#confirmation");
const generateButton = document.querySelector("#generate");
const resultPanel = document.querySelector("#result");
const publicKeyOutput = document.querySelector("#public-key");
const download = document.querySelector("#download");
const status = document.querySelector("#ceremony-status");
let downloadUrl = null;

function setStatus(message, kind = "") {
  status.textContent = message;
  status.dataset.kind = kind;
}

function base64Url(bytes) {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/u, "");
}

async function encrypt(seed, passphrase, publicKey) {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const passphraseBytes = new TextEncoder().encode(passphrase);
  try {
    const material = await crypto.subtle.importKey("raw", passphraseBytes, "PBKDF2", false, ["deriveKey"]);
    const key = await crypto.subtle.deriveKey(
      { name: "PBKDF2", hash: "SHA-256", salt, iterations: 600000 },
      material,
      { name: "AES-GCM", length: 256 },
      false,
      ["encrypt"],
    );
    const ciphertext = new Uint8Array(await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, seed));
    return {
      "aithos-keystore": "1.0.0",
      public_key: publicKey,
      kdf: { name: "PBKDF2", hash: "SHA-256", iterations: 600000, salt: base64Url(salt) },
      cipher: { name: "AES-GCM", iv: base64Url(iv), ciphertext: base64Url(ciphertext) },
    };
  } finally {
    passphraseBytes.fill(0);
    salt.fill(0);
    iv.fill(0);
  }
}

generateButton.addEventListener("click", async () => {
  generateButton.disabled = true;
  resultPanel.classList.add("hidden");
  setStatus("Generating and encrypting locally…");
  const passphrase = passphraseInput.value;
  const confirmation = confirmationInput.value;
  passphraseInput.value = "";
  confirmationInput.value = "";
  let seed = null;
  try {
    if (passphrase.length < 12) throw new Error("Use a passphrase of at least 12 characters");
    if (passphrase !== confirmation) throw new Error("The passphrase confirmation does not match");
    seed = crypto.getRandomValues(new Uint8Array(32));
    const signerInput = seed.slice();
    const signer = new DelegateSigner(signerInput);
    signerInput.fill(0);
    const publicKey = signer.public_key();
    signer.free();
    const keystore = await encrypt(seed, passphrase, publicKey);
    if (downloadUrl) URL.revokeObjectURL(downloadUrl);
    downloadUrl = URL.createObjectURL(new Blob(
      [JSON.stringify(keystore, null, 2) + "\n"],
      { type: "application/json" },
    ));
    download.href = downloadUrl;
    publicKeyOutput.textContent = publicKey;
    resultPanel.classList.remove("hidden");
    setStatus("Encrypted keystore ready. Save it before continuing.", "ok");
  } catch (error) {
    setStatus(error instanceof Error ? error.message : "Enrollment failed", "error");
  } finally {
    if (seed) seed.fill(0);
    generateButton.disabled = false;
  }
});

window.addEventListener("pagehide", () => {
  if (downloadUrl) URL.revokeObjectURL(downloadUrl);
});

try {
  await init();
  setStatus("Local verifier ready. Choose a new passphrase.", "ok");
} catch (_error) {
  generateButton.disabled = true;
  setStatus("The local verifier could not be loaded.", "error");
}
