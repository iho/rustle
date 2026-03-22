/// Simple NEAR staking pool with per-epoch reward distribution.
///
/// Rustle will flag:
///   - state-change-before-call  in `unstake`       (balance zeroed before async withdrawal)
///   - storage-gas               in `stake`         (inserts into UnorderedMap without gas check)
///   - round                     in `pending_reward` (float arithmetic in financial math)
///   - prepaid-gas               in `on_withdraw`   (callback doesn't check prepaid gas)
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::{env, ext_contract, log, near_bindgen, AccountId, Gas, Promise, PromiseResult};

pub const TGAS: u64 = 1_000_000_000_000;
/// Annual reward rate: 10%
const REWARD_RATE: f64 = 0.10;
/// Epoch length in nanoseconds (roughly 12 hours on NEAR mainnet)
const EPOCH_NS: u64 = 43_200_000_000_000;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakerInfo {
    pub balance: u128,
    pub reward_debt: u128,
    pub start_epoch: u64,
}

#[ext_contract(ext_self)]
pub trait SelfContract {
    fn on_withdraw(&mut self, staker: AccountId, amount: u128);
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakingPool {
    pub owner_id: AccountId,
    pub total_staked: u128,
    pub stakers: UnorderedMap<AccountId, StakerInfo>,
}

impl Default for StakingPool {
    fn default() -> Self {
        Self {
            owner_id: env::predecessor_account_id(),
            total_staked: 0,
            stakers: UnorderedMap::new(b"s"),
        }
    }
}

#[near_bindgen]
impl StakingPool {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self { owner_id, total_staked: 0, stakers: UnorderedMap::new(b"s") }
    }

    /// Stake attached NEAR.
    ///
    /// BUG (storage-gas): `self.stakers.insert` expands contract storage but
    /// the function does not check whether enough gas is attached for the
    /// storage expansion.
    #[payable]
    pub fn stake(&mut self) {
        let amount = env::attached_deposit();
        assert!(amount > 0, "must attach NEAR to stake");
        let caller = env::predecessor_account_id();
        let epoch = env::block_timestamp() / EPOCH_NS;

        let mut info = self.stakers.get(&caller).unwrap_or(StakerInfo {
            balance: 0,
            reward_debt: 0,
            start_epoch: epoch,
        });
        info.balance += amount;
        self.stakers.insert(&caller, &info); // storage-gas: no gas check
        self.total_staked += amount;
        log!("staked {} yN by {}", amount, caller);
    }

    /// Return pending reward for `staker` as a float.
    ///
    /// BUG (round): uses f64 arithmetic — rounding errors accumulate and can
    /// be exploited in financial calculations.
    pub fn pending_reward(&self, staker: AccountId) -> u128 {
        let info = match self.stakers.get(&staker) {
            Some(i) => i,
            None => return 0,
        };
        let current_epoch = env::block_timestamp() / EPOCH_NS;
        let epochs_elapsed = current_epoch.saturating_sub(info.start_epoch) as f64;
        // BUG: f64 rounding in financial math
        let reward = (info.balance as f64) * REWARD_RATE * (epochs_elapsed / 365.0);
        reward as u128
    }

    /// Unstake and withdraw all staked NEAR plus rewards.
    ///
    /// BUG (state-change-before-call): `staker.balance` is set to 0 and the
    /// entry removed before the async withdrawal. If the transfer fails, the
    /// staker's funds are lost because the state change is not rolled back.
    pub fn unstake(&mut self) -> Promise {
        let caller = env::predecessor_account_id();
        let mut info = self.stakers.get(&caller).expect("not staked");
        let reward = self.pending_reward(caller.clone());
        let total = info.balance + reward;

        // BUG: state changed before the cross-contract call
        info.balance = 0;
        self.stakers.remove(&caller); // removed before transfer confirmed
        self.total_staked -= total.min(self.total_staked);

        Promise::new(caller.clone())
            .transfer(total)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(Gas(5 * TGAS))
                    .on_withdraw(caller, total),
            )
    }

    /// Callback after withdrawal.
    ///
    /// BUG (prepaid-gas): does not verify `env::prepaid_gas()` is sufficient
    /// before performing cleanup, which could cause an out-of-gas panic.
    #[private]
    pub fn on_withdraw(&mut self, staker: AccountId, amount: u128) {
        // BUG: no prepaid gas check
        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                log!("withdrew {} yN for {}", amount, staker);
            }
            _ => {
                log!("withdrawal failed for {}, re-crediting", staker);
                // Attempt to restore state, but this is already broken
                self.stakers.insert(
                    &staker,
                    &StakerInfo { balance: amount, reward_debt: 0, start_epoch: 0 },
                );
                self.total_staked += amount;
            }
        }
    }

    pub fn get_staked(&self, staker: AccountId) -> u128 {
        self.stakers.get(&staker).map(|i| i.balance).unwrap_or(0)
    }

    pub fn total_staked(&self) -> u128 {
        self.total_staked
    }
}
