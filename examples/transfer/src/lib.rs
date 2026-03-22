use near_contract_standards::fungible_token::core::ext_ft_core;
use near_sdk::json_types::U128;
use near_sdk::{env, ext_contract, near, near_bindgen, AccountId, Gas, Promise, PromiseResult};

pub const TGAS: u64 = 1_000_000_000_000;
const GAS_FOR_FT_TRANSFER: Gas = Gas::from_gas(10 * TGAS);
const GAS_FOR_WITHDRAW_CALLBACK: Gas = Gas::from_gas(20 * TGAS);

#[ext_contract(ext_self)]
pub trait SelfContract {
    fn callback_withdraw(&mut self, amount: U128);
}

#[near(contract_state)]
pub struct Contract {
    token_id: AccountId,
    depositor: AccountId,
    balance: u128,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            balance: 100,
            token_id: "ft_token.near".parse().unwrap(),
            depositor: "depositor.near".parse().unwrap(),
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn withdraw_nep141(&mut self, amount: U128) -> Promise {
        assert!(self.balance >= amount.into(), "insufficient balance");

        self.balance -= amount.0;

        ext_ft_core::ext(self.token_id.clone())
            .with_attached_deposit(near_sdk::NearToken::from_yoctonear(1))
            .with_static_gas(GAS_FOR_FT_TRANSFER)
            .ft_transfer(self.depositor.clone(), amount, None)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_WITHDRAW_CALLBACK)
                    .callback_withdraw(amount),
            )
    }

    pub fn withdraw_native(&mut self, amount: U128) -> Promise {
        assert!(self.balance >= amount.into(), "insufficient balance");

        self.balance -= amount.0;

        Promise::new(self.depositor.clone())
            .transfer(near_sdk::NearToken::from_yoctonear(amount.0))
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_WITHDRAW_CALLBACK)
                    .callback_withdraw(amount),
            )
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
}
