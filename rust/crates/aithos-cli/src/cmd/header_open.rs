//! `aithos header-open` — open one's line in a header JSON and print the
//! node key.

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub file: String,
    #[arg(long)]
    pub subject_did: String,
    #[arg(long)]
    pub kid: String,
    /// The subject's `owner_kex` in multibase — `keys.kex` of its DID
    /// document, copied verbatim. I3 (spec 03.1) is checked against it before
    /// anything is opened: the owner line is the line declaring THIS kid, and
    /// the routing label never satisfies I3.
    #[arg(long)]
    pub owner_kid: String,
    #[arg(long)]
    pub sk_hex: String,
    #[arg(long, default_value_t = 1)]
    pub version: u64,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    use aithos_core::header::Header;
    let Args {
        file,
        subject_did,
        kid,
        owner_kid,
        sk_hex,
        version,
    } = args;
    let header: Header = serde_json::from_str(&std::fs::read_to_string(file)?)?;
    header.validate(&owner_kid)?;
    let sk: [u8; 32] = hex::decode(sk_hex)?
        .try_into()
        .map_err(|_| "sk-hex: expected 32 bytes")?;
    let dk = header.open(&subject_did, version, &kid, &sk.into())?;
    println!("{}", hex::encode(dk));
    Ok(())
}
