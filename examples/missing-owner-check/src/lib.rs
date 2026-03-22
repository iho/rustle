use near_sdk::{env, near, near_bindgen, AccountId, Promise};

#[near(contract_state)]
pub struct Contract {
    owner_id: AccountId,
    fee: u128,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            owner_id: env::predecessor_account_id(),
            fee: 0,
        }
    }
}

#[near_bindgen]
impl Contract {
    /// UNSAFE: any caller can drain the contract balance.
    pub fn withdraw_all(&mut self) -> Promise {
        let amount = env::account_balance();
        Promise::new(env::predecessor_account_id()).transfer(amount)
    }

    /// SAFE: only the owner can withdraw.
    pub fn withdraw_all_safe(&mut self) -> Promise {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "only owner"
        );
        let amount = env::account_balance();
        Promise::new(self.owner_id.clone()).transfer(amount)
    }

    /// UNSAFE: any caller can change the fee.
    pub fn set_fee(&mut self, new_fee: u128) {
        self.fee = new_fee;
    }

    /// SAFE: only the owner can change the fee.
    pub fn set_fee_safe(&mut self, new_fee: u128) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "only owner"
        );
        self.fee = new_fee;
    }
}
