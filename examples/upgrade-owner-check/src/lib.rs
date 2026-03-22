use near_sdk::{env, near, near_bindgen, AccountId};
use near_sys as sys;

#[near(contract_state)]
pub struct Contract {
    pub owner_id: AccountId,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            owner_id: "owner.near".parse().unwrap(),
        }
    }
}

#[near_bindgen]
impl Contract {
    /// Safe read-only method.
    pub fn get_owner(&self) -> AccountId {
        self.owner_id.clone()
    }

    /// BUG: Upgrade function with no owner check.
    ///
    /// Anyone can call this method to deploy arbitrary bytecode and take over
    /// the contract.  A proper implementation would start with:
    ///   require!(env::predecessor_account_id() == self.owner_id, "Owner only");
    pub fn upgrade(&self) {
        let current_id = env::current_account_id();
        let current_id_bytes = current_id.as_bytes();
        unsafe {
            sys::input(0);
            let promise_id = sys::promise_batch_create(
                current_id_bytes.len() as _,
                current_id_bytes.as_ptr() as _,
            );
            sys::promise_batch_action_deploy_contract(promise_id, u64::MAX as _, 0);
            let migrate = b"migrate";
            sys::promise_batch_action_function_call(
                promise_id,
                migrate.len() as _,
                migrate.as_ptr() as _,
                0,
                0,
                0,
                10_000_000_000_000_u64,
            );
            sys::promise_return(promise_id);
        }
    }
}
