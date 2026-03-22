use near_sdk::json_types::U128;
use near_sdk::{env, near, near_bindgen, AccountId, Gas, Promise, PromiseResult};

pub const TGAS: u64 = 1_000_000_000_000;
const GAS_FOR_WITHDRAW_CALLBACK: Gas = Gas::from_gas(20 * TGAS);

#[near(contract_state)]
pub struct Contract {
    depositor: AccountId,
    balance: u128,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            balance: 100,
            depositor: "depositor.near".parse().unwrap(),
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn withdraw(&mut self, amount: U128) -> Promise {
        self.sub_balance(amount.into());

        Promise::new(self.depositor.clone())
            .transfer(near_sdk::NearToken::from_yoctonear(amount.0))
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_WITHDRAW_CALLBACK)
                    .callback_withdraw(amount),
            )
    }

    pub fn check_balance(&self) -> U128 {
        U128(self.balance)
    }

    #[private]
    pub fn callback_withdraw(&mut self, amount: U128) {
        match env::promise_result(0) {
            PromiseResult::Successful(_) => {}
            PromiseResult::Failed => {
                self.balance += amount.0;
            }
        };
    }

    fn sub_balance(&mut self, amount: u128) {
        assert!(self.balance >= amount, "insufficient balance");
        self.balance -= amount;
    }
}
