use std::fmt;
use std::hash::{Hash, Hasher};

use blake2::Blake2b;
use digest::{self, Input, VariableOutput};

use byteorder::{BigEndian, LittleEndian, ByteOrder};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Network {
    Test,
    Beta,
    Main,
}

fn write_hex(f: &mut fmt::Formatter, bytes: &[u8]) -> fmt::Result {
    for b in bytes.iter() {
        write!(f, "{:02X}", b)?;
    }
    Ok(())
}

pub use ed25519_dalek::Signature;

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub signature: Signature,
    pub work: [u8; 8], // == u64
}

#[derive(Default, PartialEq, Eq, Clone)]
pub struct BlockHash(pub [u8; 32]);

impl fmt::Debug for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write_hex(f, &self.0)
    }
}

impl Hash for BlockHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0);
    }
}

#[derive(Default, PartialEq, Eq, Clone)]
pub struct Account(pub [u8; 32]);

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write_hex(f, &self.0)
    }
}

impl Hash for Account {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0);
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BlockInner {
    Send {
        previous: BlockHash,
        destination: Account,
        /// The balance of the account *after* the send.
        balance: u128,
    },
    Receive {
        previous: BlockHash,
        /// The block we're receiving.
        source: BlockHash,
    },
    /// The first "receive" in an account chain.
    /// Creates the account, and sets the representative.
    Open {
        /// The block we're receiving.
        source: BlockHash,
        representative: Account,
        account: Account,
    },
    /// Changes the representative for an account.
    Change {
        previous: BlockHash,
        representative: Account,
    },
}

impl Hash for BlockInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            BlockInner::Send {
                ref previous,
                ref destination,
                ref balance,
            } => {
                previous.hash(state);
                destination.hash(state);
                let mut buf = [0u8; 16];
                BigEndian::write_u128(&mut buf, *balance);
                state.write(&buf);
            }
            BlockInner::Receive {
                ref previous,
                ref source,
            } => {
                previous.hash(state);
                source.hash(state);
            }
            BlockInner::Open {
                ref source,
                ref representative,
                ref account,
            } => {
                source.hash(state);
                representative.hash(state);
                account.hash(state);
            }
            BlockInner::Change {
                ref previous,
                ref representative,
            } => {
                previous.hash(state);
                representative.hash(state);
            }
        }
    }
}

struct DigestHasher<'a, D: 'a>(&'a mut D);

impl<'a, D: digest::Input + 'a> Hasher for DigestHasher<'a, D> {
    fn write(&mut self, input: &[u8]) {
        self.0.process(input);
    }

    fn finish(&self) -> u64 {
        unimplemented!()
    }
}

impl BlockInner {
    pub fn get_hash(&self) -> BlockHash {
        let mut hasher = Blake2b::new(32).expect("Unsupported hash length");
        self.hash(&mut DigestHasher(&mut hasher));
        let mut output = BlockHash::default();
        hasher.variable_result(&mut output.0).expect("Incorrect hash length");
        output
    }
}

pub enum BlockRoot {
    Block(BlockHash),
    Account(Account),
}

impl Into<[u8; 32]> for BlockRoot {
    fn into(self) -> [u8; 32] {
        match self {
            BlockRoot::Block(BlockHash(bytes)) => bytes,
            BlockRoot::Account(Account(bytes)) => bytes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub inner: BlockInner,
    pub header: BlockHeader,
}

impl Block {
    pub fn get_hash(&self) -> BlockHash {
        self.inner.get_hash()
    }

    #[allow(dead_code)]
    pub fn root(&self) -> BlockRoot {
        match self.inner {
            BlockInner::Send { ref previous, .. } => BlockRoot::Block(previous.clone()),
            BlockInner::Receive { ref previous, .. } => BlockRoot::Block(previous.clone()),
            BlockInner::Change { ref previous, .. } => BlockRoot::Block(previous.clone()),
            BlockInner::Open { ref account, .. } => BlockRoot::Account(account.clone()),
        }
    }

    pub fn root_bytes(&self) -> &[u8; 32] {
        match self.inner {
            BlockInner::Send { ref previous, .. } => &previous.0,
            BlockInner::Receive { ref previous, .. } => &previous.0,
            BlockInner::Change { ref previous, .. } => &previous.0,
            BlockInner::Open { ref account, .. } => &account.0,
        }
    }

    pub fn work_threshold(&self, network: Network) -> u64 {
        match network {
            Network::Main | Network::Beta => 0xffffffc000000000,
            Network::Test => 0xff00000000000000,
        }
    }

    pub fn work_value(&self) -> u64 {
        let mut output = [0u8; 8];
        let mut hasher = Blake2b::new(output.len()).expect("Unsupported hash length");
        hasher.process(&self.header.work);
        hasher.process(self.root_bytes() as _);
        hasher.variable_result(&mut output).unwrap();
        LittleEndian::read_u64(&output as _)
    }

    pub fn work_valid(&self, network: Network) -> bool {
        self.work_value() > self.work_threshold(network)
    }
}

impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl Eq for Block {}
