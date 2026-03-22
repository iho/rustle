use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use borsh::BorshSerialize;
use near_sdk::collections::LookupMap;
use near_sdk::{
    assert_one_yocto, env, log, near, near_bindgen, AccountId, BorshStorageKey, NearToken, Promise,
    StorageUsage,
};
type Balance = u128;

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Accounts,
}

#[near(contract_state)]
pub struct Contract {
    /// AccountID -> Account balance.
    pub accounts: LookupMap<AccountId, Balance>,

    /// Total supply of the all token.
    pub total_supply: u128,

    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new() -> Self {
        Self {
            accounts: LookupMap::new(StorageKey::Accounts),
            total_supply: 0,
            account_storage_usage: 0, // 0 for empty account
        }
    }
}

impl Contract {
    fn internal_storage_balance_of(&self, account_id: &AccountId) -> Option<StorageBalance> {
        if self.accounts.contains_key(account_id) {
            Some(StorageBalance {
                total: self.storage_balance_bounds().min,
                available: NearToken::from_yoctonear(0),
            })
        } else {
            None
        }
    }
}

impl StorageManagement for Contract {
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        #[allow(unused_variables)] registration_only: Option<bool>,
    ) -> StorageBalance {
        let amount: Balance = env::attached_deposit().as_yoctonear();
        let account_id = account_id.unwrap_or_else(env::predecessor_account_id);
        if self.accounts.contains_key(&account_id) {
            log!("The account is already registered, refunding the deposit");
            if amount > 0 {
                Promise::new(env::predecessor_account_id()).transfer(NearToken::from_yoctonear(amount));
            }
        } else {
            let min_balance = self.storage_balance_bounds().min.as_yoctonear();
            if amount < min_balance {
                env::panic_str("The attached deposit is less than the minimum storage balance");
            }

            if self.accounts.insert(&account_id, &0).is_some() {
                env::panic_str("The account is already registered");
            }

            let refund = amount - min_balance;
            if refund > 0 {
                Promise::new(env::predecessor_account_id()).transfer(NearToken::from_yoctonear(refund));
            }
        }
        self.internal_storage_balance_of(&account_id).unwrap()
    }

    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        assert_one_yocto();
        let predecessor_account_id = env::predecessor_account_id();
        if let Some(storage_balance) = self.internal_storage_balance_of(&predecessor_account_id) {
            match amount {
                Some(amount) if amount.as_yoctonear() > 0 => {
                    env::panic_str("The amount is greater than the available storage balance");
                }
                _ => storage_balance, // refund all if amount is None
            }
        } else {
            env::panic_str(
                format!("The account {} is not registered", &predecessor_account_id).as_str(),
            );
        }
    }

    /// This function doesn't check whether the balance is not zero before removing the account.
    /// Such behavior is incorrect.
    #[allow(unused)]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let force = force.unwrap_or(false);
        if let Some(balance) = self.accounts.get(&account_id) {
            self.accounts.remove(&account_id);
            self.total_supply -= balance;
            Promise::new(account_id.clone()).transfer(NearToken::from_yoctonear(self.storage_balance_bounds().min.as_yoctonear() + 1));
            return true;
        } else {
            log!("The account {} is not registered", &account_id);
        }
        false
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        let required_storage_balance =
            Balance::from(self.account_storage_usage) * env::storage_byte_cost().as_yoctonear();

        StorageBalanceBounds {
            min: NearToken::from_yoctonear(required_storage_balance),
            max: Some(NearToken::from_yoctonear(required_storage_balance)),
        }
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.internal_storage_balance_of(&account_id)
    }
}
