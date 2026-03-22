use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::serde::Serialize;
use near_sdk::{near, near_bindgen, AccountId, BorshStorageKey, Timestamp};
type Balance = u128;

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Stakes,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Stake {
    owner: AccountId,

    // both u64 and u128 are too large for json
    amount: u128,     // u128
    pub start_date: Timestamp, // u64
    pub end_date: Timestamp,   // u64
}

#[near(contract_state)]
pub struct Contract {
    stakes: UnorderedMap<u16, Stake>,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            stakes: UnorderedMap::new(StorageKey::Stakes),
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_stake_info(&self, id: u16) -> Stake {
        let stake = self.stakes.get(&id).unwrap();
        stake
    }
}
