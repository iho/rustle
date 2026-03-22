use near_sdk::{env, near, near_bindgen, Gas, Promise};

#[near(contract_state)]
pub struct Contract {
    credits: u128,
}

impl Default for Contract {
    fn default() -> Self {
        Self { credits: 0 }
    }
}

#[near_bindgen]
impl Contract {
    /// Initiates a cross-contract call.
    pub fn buy_credits(&mut self, amount: u128) -> Promise {
        Promise::new("oracle.near".parse().unwrap())
            .function_call("get_price".to_string(), vec![], near_sdk::NearToken::from_yoctonear(0), Gas::from_gas(env::prepaid_gas().as_gas() / 2))
            .then(Self::ext(env::current_account_id()).on_price(amount))
    }

    /// UNSAFE: reads the promise result but never checks whether it succeeded.
    /// Credits are always applied even when the upstream call failed.
    #[private]
    pub fn on_price_unsafe(&mut self, amount: u128) {
        let count = env::promise_results_count();
        // Uses count to index into results but never calls is_promise_success
        for i in 0..count {
            let _result = env::promise_result(i);
        }
        self.credits += amount;
    }

    /// SAFE: verifies the promise succeeded before crediting.
    #[private]
    pub fn on_price(&mut self, amount: u128) {
        if !near_sdk::is_promise_success() {
            env::panic_str("upstream call failed");
        }
        self.credits += amount;
    }
}
