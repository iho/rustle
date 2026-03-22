use near_sdk::{env, near, near_bindgen, Promise, PromiseResult};

#[near(contract_state)]
pub struct Contract {
    value: u64,
}

impl Default for Contract {
    fn default() -> Self {
        Self { value: 0 }
    }
}

#[near_bindgen]
impl Contract {
    /// Combines two cross-contract calls and chains a callback.
    pub fn call_two(&self) -> Promise {
        let p1 = Promise::new("a.near".parse().unwrap())
            .function_call("get_a".to_string(), vec![], near_sdk::NearToken::from_yoctonear(0), near_sdk::Gas::from_gas(env::prepaid_gas().as_gas() / 3));
        let p2 = Promise::new("b.near".parse().unwrap())
            .function_call("get_b".to_string(), vec![], near_sdk::NearToken::from_yoctonear(0), near_sdk::Gas::from_gas(env::prepaid_gas().as_gas() / 3));
        p1.and(p2).then(
            Self::ext(env::current_account_id()).on_both()
        )
    }

    /// UNSAFE: hardcodes result index 0, ignores result 1.
    #[private]
    pub fn on_both_unsafe(&mut self) {
        // Only checks result 0, completely ignoring the second promise's result.
        let result = env::promise_result(0);
        match result {
            PromiseResult::Successful(v) => {
                self.value = u64::from_le_bytes(v.try_into().unwrap_or([0u8; 8]));
            }
            _ => env::panic_str("first call failed"),
        }
    }

    /// SAFE: iterates over all results.
    #[private]
    pub fn on_both(&mut self) {
        let count = env::promise_results_count();
        for i in 0..count {
            let result = env::promise_result(i);
            match result {
                PromiseResult::Successful(_) => {}
                _ => env::panic_str("a call failed"),
            }
        }
        self.value += 1;
    }
}
