//! Injected randomness (§00 purity rule): the bundle never decides where its
//! entropy comes from — surfaces (CLI, tests) do.

/// Source of entropy for DKs, sids, ephemerals and nonces.
pub trait EntropySource {
    fn fill(&mut self, buf: &mut [u8]);

    fn e32(&mut self) -> [u8; 32] {
        let mut b = [0u8; 32];
        self.fill(&mut b);
        b
    }
    fn e24(&mut self) -> [u8; 24] {
        let mut b = [0u8; 24];
        self.fill(&mut b);
        b
    }
    fn e16(&mut self) -> [u8; 16] {
        let mut b = [0u8; 16];
        self.fill(&mut b);
        b
    }
}

/// Deterministic source for tests and vector replay: BLAKE3 of a counter.
#[derive(Debug, Default)]
pub struct SeqEntropy {
    counter: u64,
}

impl EntropySource for SeqEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        let mut out = Vec::new();
        while out.len() < buf.len() {
            self.counter += 1;
            out.extend_from_slice(
                blake3::hash(format!("seq-entropy-{}", self.counter).as_bytes()).as_bytes(),
            );
        }
        buf.copy_from_slice(&out[..buf.len()]);
    }
}

/// OS randomness for real surfaces.
#[derive(Debug, Default)]
pub struct OsEntropy;

impl EntropySource for OsEntropy {
    fn fill(&mut self, buf: &mut [u8]) {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(buf))
            .expect("OS randomness available");
    }
}
