/// NFT Marketplace — list, buy, and delist NFTs for a fixed NEAR price.
///
/// Rustle will flag:
///   - missing-owner-check  in `set_fee_rate`     (transfers NEAR without owner check)
///   - nft-approval-check   in `buy`              (approval_id not verified on transfer)
///   - yocto-attach         in `delist`           (privileged fn missing assert_one_yocto)
///   - unchecked-promise-result in `on_nft_transfer` (doesn't call is_promise_success)
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::U128;
use near_sdk::{env, ext_contract, log, near_bindgen, AccountId, Gas, Promise, PromiseResult};

#[ext_contract(ext_nft)]
pub trait NftContract {
    fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: Option<u64>,
        memo: Option<String>,
    );
}

pub const TGAS: u64 = 1_000_000_000_000;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Listing {
    pub seller: AccountId,
    pub nft_contract: AccountId,
    pub token_id: String,
    pub price: u128,
}

#[ext_contract(ext_self)]
pub trait SelfContract {
    fn on_nft_transfer(&mut self, listing_key: String, buyer: AccountId, price: u128);
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Marketplace {
    pub owner_id: AccountId,
    /// fee in basis points (e.g. 250 = 2.5%)
    pub fee_bps: u32,
    /// key: "<nft_contract>:<token_id>"
    pub listings: UnorderedMap<String, Listing>,
}

impl Default for Marketplace {
    fn default() -> Self {
        Self {
            owner_id: env::predecessor_account_id(),
            fee_bps: 250,
            listings: UnorderedMap::new(b"l"),
        }
    }
}

#[near_bindgen]
impl Marketplace {
    #[init]
    pub fn new(owner_id: AccountId, fee_bps: u32) -> Self {
        Self { owner_id, fee_bps, listings: UnorderedMap::new(b"l") }
    }

    /// List an NFT for sale.
    pub fn list_nft(&mut self, nft_contract: AccountId, token_id: String, price: U128) {
        let key = format!("{}:{}", nft_contract, token_id);
        self.listings.insert(
            &key,
            &Listing {
                seller: env::predecessor_account_id(),
                nft_contract,
                token_id,
                price: price.0,
            },
        );
        log!("listed: {} for {} yN", key, price.0);
    }

    /// Delist an NFT (seller or owner only).
    ///
    /// BUG (yocto-attach): privileged operation (only seller/owner) should call
    /// `assert_one_yocto()` to prevent CSRF-style calls from other contracts.
    pub fn delist(&mut self, nft_contract: AccountId, token_id: String) {
        // missing assert_one_yocto!()
        let key = format!("{}:{}", nft_contract, token_id);
        let listing = self.listings.get(&key).expect("not listed");
        let caller = env::predecessor_account_id();
        assert!(caller == listing.seller || caller == self.owner_id, "not authorized");
        self.listings.remove(&key);
        log!("delisted: {}", key);
    }

    /// Update the marketplace fee rate.
    ///
    /// BUG (missing-owner-check): this sends accumulated fees to `self.owner_id`
    /// but never verifies that the caller IS the owner — any account can trigger
    /// the payout.
    pub fn set_fee_rate(&mut self, new_fee_bps: u32) -> Promise {
        self.fee_bps = new_fee_bps;
        // Sends accumulated fees without checking caller == owner_id
        let balance = env::account_balance();
        Promise::new(self.owner_id.clone()).transfer(balance / 10)
    }

    /// Purchase a listed NFT.
    ///
    /// BUG (nft-approval-check): `nft_transfer` is called without supplying or
    /// verifying the approval_id, so anyone who obtained approval can front-run
    /// the transfer.
    #[payable]
    pub fn buy(&mut self, nft_contract: AccountId, token_id: String) -> Promise {
        let key = format!("{}:{}", nft_contract, token_id);
        let listing = self.listings.get(&key).expect("not listed");
        assert!(env::attached_deposit() >= listing.price, "insufficient deposit");

        let buyer = env::predecessor_account_id();

        // BUG: approval_id not provided/checked
        ext_nft::ext(listing.nft_contract.clone())
            .with_attached_deposit(1)
            .with_static_gas(Gas(30 * TGAS))
            .nft_transfer(buyer.clone(), listing.token_id.clone(), None, None)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(Gas(10 * TGAS))
                    .on_nft_transfer(key, buyer, listing.price),
            )
    }

    /// Callback after NFT transfer.
    ///
    /// BUG (unchecked-promise-result): pays the seller even if the NFT transfer
    /// failed, because it never checks `env::promise_result(0)`.
    #[private]
    pub fn on_nft_transfer(&mut self, listing_key: String, _buyer: AccountId, price: u128) {
        // BUG: should check env::promise_result(0) here
        let listing = self.listings.remove(&listing_key).expect("listing removed");
        let fee = price * self.fee_bps as u128 / 10_000;
        let seller_payout = price - fee;

        Promise::new(listing.seller.clone()).transfer(seller_payout);
        Promise::new(self.owner_id.clone()).transfer(fee);
        log!("sale: seller gets {} yN, fee {} yN", seller_payout, fee);
    }

    pub fn get_listing(&self, nft_contract: AccountId, token_id: String) -> Option<U128> {
        let key = format!("{}:{}", nft_contract, token_id);
        self.listings.get(&key).map(|l| U128(l.price))
    }
}
