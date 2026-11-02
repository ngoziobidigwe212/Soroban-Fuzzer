use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

pub const DAY_IN_LEDGERS: u32 = 17_280;
/// Entries are extended once their TTL drops below 7 days…
pub const BUMP_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
/// …back up to 30 days.
pub const BUMP_TO: u32 = 30 * DAY_IN_LEDGERS;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    InvalidAmount = 1,
    InsufficientBalance = 2,
    Overflow = 3,
    InsufficientFunds = 4,
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum DataKey {
    Token,
    Total,
    Balance(Address),
    // Used by the buggy variants only.
    Deposit(u32),
    DepositCount,
}

#[contract]
pub struct Vault;

fn bump_instance(env: &Env) {
    env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
}

fn token(env: &Env) -> token::Client<'_> {
    let id: Address = env.storage().instance().get(&DataKey::Token).unwrap();
    token::Client::new(env, &id)
}

fn read_balance(env: &Env, who: &Address) -> i128 {
    let key = DataKey::Balance(who.clone());
    let s = env.storage().persistent();
    match s.get::<_, i128>(&key) {
        Some(b) => {
            s.extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
            b
        }
        None => 0,
    }
}

fn write_balance(env: &Env, who: &Address, amount: i128) {
    let key = DataKey::Balance(who.clone());
    let s = env.storage().persistent();
    s.set(&key, &amount);
    s.extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
}

fn total(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::Total).unwrap_or(0)
}

#[contractimpl]
impl Vault {
    pub fn __constructor(env: Env, token: Address) {
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Total, &0i128);
        bump_instance(&env);
    }

    /// Move `amount` of the token from `from` into the vault.
    pub fn deposit(env: Env, from: Address, amount: i128) -> Result<(), VaultError> {
        from.require_auth();
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }
        bump_instance(&env);
        let token = token(&env);
        if token.balance(&from) < amount {
            return Err(VaultError::InsufficientFunds);
        }
        let bal = read_balance(&env, &from)
            .checked_add(amount)
            .ok_or(VaultError::Overflow)?;
        let new_total = total(&env)
            .checked_add(amount)
            .ok_or(VaultError::Overflow)?;
        token.transfer(&from, env.current_contract_address(), &amount);
        write_balance(&env, &from, bal);
        env.storage().instance().set(&DataKey::Total, &new_total);
        Ok(())
    }

    /// Pay `amount` of `to`'s deposit back out to `to`.
    pub fn withdraw(env: Env, to: Address, amount: i128) -> Result<(), VaultError> {
        to.require_auth();
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }
        bump_instance(&env);
        let bal = read_balance(&env, &to);
        if bal < amount {
            return Err(VaultError::InsufficientBalance);
        }
        write_balance(&env, &to, bal - amount);
        env.storage()
            .instance()
            .set(&DataKey::Total, &(total(&env) - amount));
        token(&env).transfer(&env.current_contract_address(), &to, &amount);
        Ok(())
    }

    /// Move `amount` of deposited funds from `from` to `to` inside the vault.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), VaultError> {
        from.require_auth();
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }
        bump_instance(&env);
        let from_bal = read_balance(&env, &from);
        if from_bal < amount {
            return Err(VaultError::InsufficientBalance);
        }
        write_balance(&env, &from, from_bal - amount);
        let to_bal = read_balance(&env, &to);
        write_balance(&env, &to, to_bal + amount);
        Ok(())
    }

    /// Read-only: deliberately does not extend any TTL, so checking balances
    /// from tests cannot keep entries alive and hide a lifecycle bug.
    pub fn balance(env: Env, who: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(who))
            .unwrap_or(0)
    }

    pub fn total(env: Env) -> i128 {
        total(&env)
    }
}
