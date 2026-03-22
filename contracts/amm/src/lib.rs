/// Simple constant-product AMM (x * y = k).
///
/// Rustle will flag:
///   - div-before-mul   in `get_return` (fee division before multiplication)
///   - timestamp        in `get_return` (uses block_timestamp for fee tier)
///   - reentrancy       in `swap`       (state updated after cross-contract call)
use near_contract_standards::fungible_token::core::ext_ft_core;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use near_sdk::{env, ext_contract, log, near_bindgen, AccountId, Gas, Promise, PromiseResult};

pub const TGAS: u64 = 1_000_000_000_000;
const FEE_DENOMINATOR: u128 = 10_000;

#[ext_contract(ext_self)]
pub trait SelfContract {
    fn on_swap_complete(&mut self, pool_id: u64, token_in: AccountId, amount_in: U128, amount_out: U128);
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Pool {
    pub token_a: AccountId,
    pub token_b: AccountId,
    pub reserve_a: u128,
    pub reserve_b: u128,
    /// Fee in basis points (e.g. 30 = 0.30%)
    pub fee_bps: u128,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct AmmContract {
    pub owner_id: AccountId,
    pub pools: Vec<Pool>,
}

impl Default for AmmContract {
    fn default() -> Self {
        Self {
            owner_id: env::predecessor_account_id(),
            pools: Vec::new(),
        }
    }
}

#[near_bindgen]
impl AmmContract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self { owner_id, pools: Vec::new() }
    }

    /// Create a new pool.  Only the owner can do this.
    pub fn create_pool(&mut self, token_a: AccountId, token_b: AccountId, fee_bps: u128) -> u64 {
        assert_eq!(env::predecessor_account_id(), self.owner_id, "owner only");
        self.pools.push(Pool { token_a, token_b, reserve_a: 0, reserve_b: 0, fee_bps });
        (self.pools.len() - 1) as u64
    }

    /// Compute how many token_b the caller receives for `amount_in` of token_a.
    ///
    /// BUG (div-before-mul): fee is divided before being multiplied, causing
    /// precision loss on small amounts.
    ///
    /// BUG (timestamp): fee tier switches based on block_timestamp, which
    /// validators can manipulate within a small window.
    pub fn get_return(&self, pool_id: u64, amount_in: U128) -> U128 {
        let pool = &self.pools[pool_id as usize];
        let amount_in = amount_in.0;

        // BUG: fee_bps / FEE_DENOMINATOR truncates to 0 for any fee < 10_000,
        // so the effective fee is never deducted.
        let dynamic_fee = if env::block_timestamp() % 2 == 0 {
            pool.fee_bps / FEE_DENOMINATOR * amount_in  // div-before-mul
        } else {
            pool.fee_bps * amount_in / FEE_DENOMINATOR  // correct order
        };

        let amount_after_fee = amount_in.saturating_sub(dynamic_fee);
        let numerator = amount_after_fee * pool.reserve_b;
        let denominator = pool.reserve_a + amount_after_fee;
        U128(numerator / denominator)
    }

    /// Execute a swap on pool `pool_id`, sending `amount_in` of token_a and
    /// receiving the computed amount of token_b.
    ///
    /// BUG (reentrancy): reserves are updated inside the callback, but if the
    /// callee re-enters `swap` before `on_swap_complete` runs, it sees stale
    /// reserves.
    pub fn swap(&mut self, pool_id: u64, amount_in: U128, min_amount_out: U128) -> Promise {
        let pool = &self.pools[pool_id as usize];
        let amount_out = self.get_return(pool_id, amount_in);
        assert!(amount_out.0 >= min_amount_out.0, "slippage exceeded");

        let token_b = pool.token_b.clone();

        // Send token_b to caller before updating reserves (reentrancy window).
        ext_ft_core::ext(token_b)
            .with_attached_deposit(1)
            .with_static_gas(Gas(30 * TGAS))
            .ft_transfer(env::predecessor_account_id(), amount_out, None)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(Gas(10 * TGAS))
                    .on_swap_complete(pool_id, self.pools[pool_id as usize].token_a.clone(), amount_in, amount_out),
            )
    }

    #[private]
    pub fn on_swap_complete(
        &mut self,
        pool_id: u64,
        _token_in: AccountId,
        amount_in: U128,
        amount_out: U128,
    ) {
        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                let pool = &mut self.pools[pool_id as usize];
                pool.reserve_a += amount_in.0;
                pool.reserve_b -= amount_out.0;
                log!("swap completed: in={} out={}", amount_in.0, amount_out.0);
            }
            _ => log!("swap failed, reserves unchanged"),
        }
    }

    /// Add liquidity (simplified: assumes balanced deposit).
    pub fn add_liquidity(&mut self, pool_id: u64, amount_a: U128, amount_b: U128) {
        let pool = &mut self.pools[pool_id as usize];
        pool.reserve_a += amount_a.0;
        pool.reserve_b += amount_b.0;
    }

    pub fn get_pool(&self, pool_id: u64) -> (U128, U128) {
        let p = &self.pools[pool_id as usize];
        (U128(p.reserve_a), U128(p.reserve_b))
    }
}
