use soroban_sdk::{testutils::Address as _, Address, Env};

/// Index of an actor in an [`Actors`] pool.
///
/// Addresses only exist inside an `Env`, so actions refer to actors by index
/// and the harness resolves them. This keeps actions `Clone + Debug`, cheap to
/// generate and easy to shrink (shrinking moves towards actor 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorId(pub u8);

/// A fixed pool of generated account addresses.
#[derive(Clone, Debug)]
pub struct Actors {
    addrs: Vec<Address>,
}

impl Actors {
    /// Generate `n` fresh addresses (`n` must be at least 1).
    pub fn generate(env: &Env, n: usize) -> Self {
        assert!(n > 0, "an actor pool needs at least one actor");
        Self {
            addrs: (0..n).map(|_| Address::generate(env)).collect(),
        }
    }

    /// The address for `id`. Ids wrap around, so any `ActorId` is valid.
    pub fn get(&self, id: ActorId) -> &Address {
        &self.addrs[id.0 as usize % self.addrs.len()]
    }

    /// The normalised id for `id` (useful as a model map key).
    pub fn id(&self, id: ActorId) -> ActorId {
        ActorId((id.0 as usize % self.addrs.len()) as u8)
    }

    pub fn len(&self) -> usize {
        self.addrs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.addrs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ActorId, &Address)> {
        self.addrs
            .iter()
            .enumerate()
            .map(|(i, a)| (ActorId(i as u8), a))
    }
}
