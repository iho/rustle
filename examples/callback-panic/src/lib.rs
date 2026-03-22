use near_sdk::store::LookupMap;
use near_sdk::{env, ext_contract, near, near_bindgen, AccountId, BorshStorageKey, PromiseResult};

#[ext_contract(ext_ft)]
pub trait FungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: String, memo: Option<String>);
}

#[derive(BorshStorageKey)]
#[near]
enum StorageKey {
    Deposits,
}

#[near(contract_state)]
pub struct Contract {
    pub deposits: LookupMap<AccountId, u128>,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            deposits: LookupMap::new(StorageKey::Deposits),
        }
    }
}

#[near_bindgen]
impl Contract {
    /// Start a withdrawal: record pending amount, then call the FT contract.
    pub fn withdraw(&mut self, amount: u128) {
        let sender = env::predecessor_account_id();
        let bal = self.deposits.get(&sender).copied().unwrap_or(0);
        assert!(bal >= amount, "Insufficient balance");
        self.deposits.insert(sender.clone(), bal - amount);

        ext_ft::ext("token.near".parse().unwrap())
            .ft_transfer(sender.clone(), amount.to_string(), None)
            .then(Self::ext(env::current_account_id()).on_withdraw(sender, amount));
    }

    /// Callback: if transfer failed, restore the sender's balance.
    ///
    /// BUG: `.unwrap()` panics when the sender was removed from `deposits`
    /// between the `withdraw` call and this callback (e.g. after forced exit).
    /// If the callback panics, NEAR rolls back the restore inside the callback,
    /// but the `ft_transfer` that already *failed* is committed — leaving the
    /// sender's on-chain balance permanently decremented with no refund.
    #[private]
    pub fn on_withdraw(&mut self, sender: AccountId, amount: u128) {
        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                // Transfer succeeded — nothing to revert.
            }
            PromiseResult::Failed => {
                // BUG: panics if sender was deregistered before this callback runs.
                let current = *self.deposits.get(&sender).unwrap();
                self.deposits.insert(sender, current + amount);
            }
        }
    }
}
