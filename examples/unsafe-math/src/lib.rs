use near_sdk::{near, near_bindgen};

#[near(contract_state)]
pub struct CalculatorContract {}

impl Default for CalculatorContract {
    fn default() -> Self {
        Self {}
    }
}

#[near_bindgen]
impl CalculatorContract {
    pub fn sum(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    pub fn sub(&self, a: i32, b: i32) -> i32 {
        a - b
    }

    pub fn mul(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
