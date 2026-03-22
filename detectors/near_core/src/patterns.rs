use regex::Regex;
use std::sync::OnceLock;

fn re(pat: &str) -> Regex {
    Regex::new(pat).unwrap_or_else(|e| panic!("invalid pattern '{pat}': {e}"))
}

// ---------------------------------------------------------------------------
// Library filters (ported from near_core.h regexForLibFunc / regexForLibLoc)
// ---------------------------------------------------------------------------

/// True if the mangled function symbol belongs to a library (core/std/cargo/…).
pub fn is_lib_func(name: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(concat!(
            r"(^/cargo)|(^/rustc)",
            r"|(_ZN\d+(core|std|alloc|num_traits|solana_program|byteorder|hex|bytemuck",
            r"|borsh|enumflags2|safe_transmute|thiserror)([0-9]+|\.\.)[a-zA-Z]+)",
            r"|(serde\.\.de\.\.Deserialize)",
        ))
    })
    .is_match(name)
}

/// True if the source file path is inside cargo/rustc caches (i.e. library code).
pub fn is_lib_loc(filename: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Covers Linux (/home/…/.cargo, /root/.cargo), macOS (/Users/…/.cargo),
        // Docker (/cargo), and rustc sysroot (/rustc).
        re(r"(^/rustc)|(^/cargo)|(^/root/\.cargo)|(^/home/.+/\.cargo)|(^/Users/.+/\.cargo)|(^$)")
    })
    .is_match(filename)
}

// ---------------------------------------------------------------------------
// Named regex accessors (ported from near_core.h built-in regex constants)
// ---------------------------------------------------------------------------

pub fn round() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"[0-9]+std[0-9]+.+[0-9]+(try_round|round)[0-9]+"))
}

pub fn ext_call() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r".+near_sdk[0-9]+promise[0-9]+Promise[0-9]+function_call(_weight)?[0-9]+")
    })
}

pub fn promise_transfer() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"near_sdk[0-9]+promise[0-9]+Promise[0-9]+transfer[0-9]+"))
}

pub fn nep141_transfer() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"[0-9]+(ft_transfer(_call)?)[0-9]+"))
}

pub fn promise_result() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(concat!(
            r"(near_sdk[0-9]+environment[0-9]+env[0-9]+",
            r"(promise_result|promise_results_count)[0-9]+)",
            r"|(near_sdk[0-9]+utils[0-9]+",
            r"(is_promise_success|promise_result_as_success)[0-9]+)",
        ))
    })
}

/// Matches `env::promise_result` and `env::promise_results_count` (access without check).
pub fn promise_result_access() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_sdk[0-9]+environment[0-9]+env[0-9]+(promise_result|promise_results_count)[0-9]+")
    })
}

/// Matches `is_promise_success` and `promise_result_as_success` (the check).
pub fn promise_result_check() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_sdk[0-9]+utils[0-9]+(is_promise_success|promise_result_as_success)[0-9]+")
    })
}

pub fn partial_eq() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"core\.\.cmp\.\.PartialEq"))
}

pub fn partial_ord() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"core[0-9]+cmp[0-9]+PartialOrd"))
}

pub fn predecessor_account_id() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_sdk[0-9]+environment[0-9]+env[0-9]+predecessor_account_id")
    })
}

pub fn block_timestamp() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"block_timestamp"))
}

pub fn llvm_mul_overflow() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"llvm\.[a-z]?mul\.with\.overflow\."))
}

pub fn prepaid_gas() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_sdk[0-9]+environment[0-9]+env[0-9]+prepaid_gas")
    })
}

pub fn ft_transfer_call_standard() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_contract_standards.+fungible_token.+core_impl.+FungibleToken.+[0-9](ft_transfer(_call)?|internal_transfer)[0-9]")
    })
}

pub fn ft_transfer_trait() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_contract_standards\.\.fungible_token\.\.core\.\.FungibleTokenCore\$.+[0-9]ft_transfer[0-9]")
    })
}

pub fn ft_transfer_call_trait() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_contract_standards\.\.fungible_token\.\.core\.\.FungibleTokenCore\$.+[0-9]ft_transfer_call[0-9]")
    })
}

// ---------------------------------------------------------------------------
// upgrade_func patterns
// ---------------------------------------------------------------------------

pub fn promise_batch_deploy() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"promise_batch_action_deploy_contract"))
}

pub fn promise_batch_function_call() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"promise_batch_action_function_call"))
}

// ---------------------------------------------------------------------------
// storage_gas patterns
// ---------------------------------------------------------------------------

pub fn storage_expansion() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"near_sdk[0-9]+collections[0-9]+.+(insert|extend)"))
}

pub fn storage_usage() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_sdk[0-9]+environment[0-9]+env[0-9]+storage_usage[0-9]+")
    })
}

// ---------------------------------------------------------------------------
// unsaved_changes / unregistered_receiver patterns
// ---------------------------------------------------------------------------

/// Matches `get` calls on NEAR SDK map collections.
pub fn map_get() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(concat!(
            r"near_sdk[0-9]+collections[0-9]+",
            r"(lookup_map[0-9]+LookupMap|tree_map[0-9]+TreeMap|",
            r"unordered_map[0-9]+UnorderedMap|legacy_tree_map[0-9]+LegacyTreeMap)",
            r"\$.+[0-9]+get[0-9]+"
        ))
    })
}

/// Matches `insert` calls on NEAR SDK map collections.
pub fn map_insert() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(concat!(
            r"near_sdk[0-9]+collections[0-9]+",
            r"(lookup_map[0-9]+LookupMap|tree_map[0-9]+TreeMap|",
            r"unordered_map[0-9]+UnorderedMap|legacy_tree_map[0-9]+LegacyTreeMap)",
            r"\$.+[0-9]+insert[0-9]+"
        ))
    })
}

/// Matches any unwrap variant on Option or Result.
pub fn all_unwrap() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(concat!(
            r"core[0-9]+option[0-9]+Option\$.+[0-9]+",
            r"(unwrap|unwrap_or|unwrap_or_else|unwrap_or_default|unwrap_unchecked)[0-9]+"
        ))
    })
}

/// Matches unsafe unwrap variants (unwrap_or / unwrap_or_default / unwrap_unchecked).
pub fn unchecked_unwrap() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(concat!(
            r"core[0-9]+(option[0-9]+Option|result[0-9]+Result)\$.+[0-9]+",
            r"(unwrap_or|unwrap_or_default|unwrap_unchecked)[0-9]+"
        ))
    })
}

// ---------------------------------------------------------------------------
// nft_owner_check / nft_approval_check patterns
// ---------------------------------------------------------------------------

pub fn nft_approve_function() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"[0-9]+(nft_approve|nft_revoke|nft_revoke_all)[0-9]+"))
}

/// Matches bare `nft_transfer` trait implementations (for nft_approval_check).
pub fn nft_transfer_bare() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"[0-9]nft_transfer[0-9]"))
}

/// Matches bare `nft_transfer_call` trait implementations.
pub fn nft_transfer_call_bare() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"[0-9]nft_transfer_call[0-9]"))
}

/// Matches the standard NonFungibleToken implementation of transfer/internal_transfer.
pub fn nft_standard_transfer() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_contract_standards.+non_fungible_token.+core_impl.+NonFungibleToken.+[0-9](nft_transfer(_call)?|internal_transfer)[0-9]")
    })
}

// ---------------------------------------------------------------------------
// unclaimed_storage_fee pattern
// ---------------------------------------------------------------------------

pub fn storage_unregister() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"[0-9]+storage_unregister[0-9]+"))
}

/// Matches any Promise method call (for unhandled_promise handling detection).
pub fn promise_any_method() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"(.+near_sdk[0-9]+promise[0-9]+Promise[0-9]+([_a-z]+)[0-9]+[0-9a-z]+)")
    })
}

/// Matches `core::ptr::drop_in_place<near_sdk::promise::Promise>`.
pub fn promise_drop_in_place() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"drop_in_place.*near_sdk.*promise.*Promise"))
}

/// Matches any `llvm.*.with.overflow.*` intrinsic (broader than llvm_mul_overflow).
pub fn with_overflow() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"llvm\..+\.with\.overflow\."))
}

/// Matches `core::num::checked_*` arithmetic operations.
pub fn checked_math() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"core.+num.+checked_"))
}

pub fn assert_one_yocto() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| re(r"near_sdk[0-9]+utils[0-9]+assert_one_yocto"))
}

pub fn account_id_eq() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        re(r"near_sdk\.\.types\.\.account_id\.\.AccountId.+core\.\.cmp\.\.PartialEq")
    })
}
