use near_sdk::json_types::U128;
use near_sdk::{ext_contract, near, near_bindgen, Promise};

#[ext_contract(ext_calculator)]
trait Calculator {
    fn sum(&self, a: U128, b: U128) -> U128;
}

#[near(contract_state)]
pub struct Contract {}

impl Default for Contract {
    fn default() -> Self {
        Self {}
    }
}

#[near_bindgen]
impl Contract {
    pub fn sum_a_b(&mut self, a: U128, b: U128) -> Promise {
        let calculator_account_id = "calc.near".parse().unwrap();
        ext_calculator::ext(calculator_account_id).sum(a, b) // run sum(a, b) on remote
    }
}
