// Find all our documentation at https://docs.near.org
use near_sdk::{
    ext_contract,
    json_types::Base64VecU8,
    serde::{Deserialize, Serialize},
    AccountId, PromiseOrValue,
};

use std::collections::HashMap;

// missing borsh serialize and de serialize
#[derive(Deserialize, Serialize)]
pub struct TokenMetadata {
    /// the Title for this token. ex. "Arch Nemesis: Mail Carrier" or "Parcel 5055"
    pub title: Option<String>,
    /// free-form description of this token.
    pub description: Option<String>,
    /// URL to associated media, preferably to decentralized, content-addressed storage
    pub media: Option<String>,
    /// Base64-encoded sha256 hash of content referenced by the `media` field.
    /// Required if `media` is included.
    pub media_hash: Option<Base64VecU8>,
    /// number of copies of this set of metadata in existence when token was minted.
    pub copies: Option<u16>,
    /// ISO 8601 datetime when token expires.
    pub expires_at: Option<String>,
    /// ISO 8601 datetime when token starts being valid.
    pub starts_at: Option<String>,
    /// When token was last updated, Unix epoch in milliseconds
    pub extra: Option<String>,
    /// URL to an off-chain JSON file with more info. The Mintbase Indexer refers
    /// to this field as `thing_id` or sometimes, `meta_id`.
    pub reference: Option<String>,
    /// Base64-encoded sha256 hash of JSON from reference field. Required if
    /// `reference` is included.
    pub reference_hash: Option<Base64VecU8>,
}

pub type SplitBetweenUnparsed = HashMap<AccountId, u32>;

/// Unparsed pre-image of a Royalty struct. Used in `Store::mint_tokens`.
#[derive(Deserialize, Serialize)]
pub struct RoyaltyArgs {
    pub split_between: SplitBetweenUnparsed,
    pub percentage: u32,
}

// Validator interface, for cross-contract calls
#[ext_contract(mintbase_nft)]
trait MintbaseNft {
    fn check_is_minter(&self, account_id: near_sdk::AccountId) -> bool;

    fn nft_batch_mint(
        &mut self,
        owner_id: near_sdk::AccountId,
        metadata: TokenMetadata,
        num_to_mint: u64,
        royalty_args: Option<RoyaltyArgs>,
        split_owners: Option<SplitBetweenUnparsed>,
    ) -> PromiseOrValue<()>;
}
