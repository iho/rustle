use near_contract_standards::fungible_token::core::ext_ft_core;
use near_sdk::json_types::U128;
use near_sdk::{env, near, near_bindgen, AccountId, Gas, Promise};

const GAS_FOR_FT_TRANSFER: Gas = Gas::from_gas(10_000_000_000_000);

#[near(contract_state)]
pub struct Contract {
    token_id: AccountId,
    receiver: AccountId,
    balance: u128,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            token_id: "token.near".parse().unwrap(),
            receiver: "receiver.near".parse().unwrap(),
            balance: 1_000_000,
        }
    }
}

#[near_bindgen]
impl Contract {
    /// UNSAFE: deducts balance then fires a cross-contract transfer with no
    /// callback.  If the transfer fails, the deducted balance is lost forever.
    pub fn withdraw_unsafe(&mut self, amount: U128) {
        assert!(self.balance >= amount.0, "insufficient balance");
        self.balance -= amount.0;
        ext_ft_core::ext(self.token_id.clone())
            .with_static_gas(GAS_FOR_FT_TRANSFER)
            .ft_transfer(self.receiver.clone(), amount, None);
    }

    /// SAFE: deducts balance and attaches a callback to restore it on failure.
    pub fn withdraw_safe(&mut self, amount: U128) -> Promise {
        assert!(self.balance >= amount.0, "insufficient balance");
        self.balance -= amount.0;
        ext_ft_core::ext(self.token_id.clone())
            .with_static_gas(GAS_FOR_FT_TRANSFER)
            .ft_transfer(self.receiver.clone(), amount, None)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_FT_TRANSFER)
                    .on_withdraw(amount),
            )
    }

    #[private]
    pub fn on_withdraw(&mut self, amount: U128) {
        if !near_sdk::is_promise_success() {
            self.balance += amount.0;
        }
    }
}
