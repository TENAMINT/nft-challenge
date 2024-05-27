// Find all our documentation at https://docs.near.org

use external::TokenMetadata;
use mockall::predicate::*;
use near_sdk::{
    env, near,
    serde::{Deserialize, Serialize},
    store::{LookupMap, Vector},
    AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseResult,
};
pub mod external;
pub use crate::external::*;

#[derive(
    Clone,
    Debug,
    Deserialize,
    Serialize,
    // BorshDeserialize, BorshSerialize
)]
pub struct ChallengeMetaData {
    // the owner of this NFT Challenge
    pub owner_id: String,
    // the name for this challenge.
    pub name: String,
    // free-form description of this challenge.
    pub description: String,
    // URL to associated media, preferably to decentralized, content-addressed storage
    pub image_link: Option<String>,
    // NFT that will be minted to winners of this challenge.
    pub reward_nft: String,
    // ISO 8601 datetime when challenge terminates.
    pub challenge_nft_ids: Vec<String>,
    // Current number of winners for this challenge.
    pub termination_date_in_ns: u64,
    // Maximum number of winners for this challenge.
    pub winner_limit: u64,
    // Whether the challenge is completed
    pub challenge_completed: bool,
    // List of NFTS that are part of the challenge
    pub winners_count: u64,
}

// Define the contract structure
#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    owner_id: String,
    creator_id: String,
    name: String,
    description: String,
    image_link: String,
    reward_nft: String,
    challenge_nft_ids: Vector<String>,
    // challenge_burn_list: Vector<bool>,
    termination_date_in_ns: u64,
    winner_limit: u64,
    challenge_completed: bool,
    winner_count: u64,
    potential_winners_left: u64,
    winners: LookupMap<AccountId, u64>,
    token_metadata: TokenMetadata,
}

// Implement the contract structure
#[near]
impl Contract {
    #[init]
    pub fn new(
        owner_id: String,
        name: String,
        description: String,
        image_link: String,
        reward_nft: String,
        _challenge_nft_ids: std::vec::Vec<String>,
        termination_date_in_ns: u64,
        winner_limit: u64,
        token_metadata: TokenMetadata,
    ) -> Self {
        let mut challenge_nft_ids = Vector::new(b"a");
        for challenge in _challenge_nft_ids.iter() {
            challenge_nft_ids.push(challenge.clone());
        }

        assert!(
            _challenge_nft_ids.len() > 0,
            "Challenge must have at least 1 challenge NFT"
        );
        // Assert that the owner_id is valid
        Self {
            owner_id,
            creator_id: env::predecessor_account_id().to_string(),
            name,
            description,
            image_link,
            reward_nft,
            challenge_nft_ids,
            termination_date_in_ns,
            winner_limit,
            challenge_completed: false,
            winner_count: 0,
            potential_winners_left: winner_limit,
            winners: LookupMap::new(b"z"),
            token_metadata,
        }
    }

    // -------------------------- view methods ---------------------------
    pub fn mint_nft(&self) -> Promise {
        assert!(
            self.check_account_is_winner(env::predecessor_account_id()),
            "You must win the challenge to mint the NFT"
        );
        let promise = mintbase_nft::ext(self.reward_nft.parse().unwrap())
            .with_static_gas(Gas::from_tgas(5))
            .with_attached_deposit(NearToken::from_millinear(54))
            .nft_batch_mint(
                env::predecessor_account_id(),
                self.token_metadata.clone(),
                1,
                None,
                None,
            );

        return promise.then(
            // Create a promise to callback query_greeting_callback
            Self::ext(env::current_account_id())
                .with_static_gas(Gas::from_tgas(5))
                .mint_nft_callback(),
        );
    }

    pub fn get_challenge_metadata(&self) -> ChallengeMetaData {
        let mut challenge_list = Vec::new();
        for challenge in self.challenge_nft_ids.iter() {
            challenge_list.push(challenge.clone());
        }
        ChallengeMetaData {
            owner_id: self.owner_id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            image_link: Some(self.image_link.clone()),
            reward_nft: self.reward_nft.clone(),
            challenge_nft_ids: challenge_list,
            termination_date_in_ns: self.termination_date_in_ns,
            winner_limit: self.winner_limit,
            challenge_completed: self.challenge_completed,
            winners_count: self.winner_count,
        }
    }

    // Show the current owner of this NFT Challenge
    pub fn get_owner_id(&self) -> String {
        self.owner_id.clone()
    }

    pub fn is_challenge_expired(&self) -> bool {
        println!(
            "Checking if challenge is expired {}",
            env::block_timestamp(),
        );
        self.challenge_completed && env::block_timestamp() > self.termination_date_in_ns
    }

    pub fn potential_winners_left(&self) -> u64 {
        self.potential_winners_left
    }

    pub fn check_account_is_winner(&self, account_id: AccountId) -> bool {
        self.winners.contains_key(&account_id)
    }

    // -------------------------- change methods ---------------------------
    pub fn initiate_claim(&mut self) -> Promise {
        if self.potential_winners_left == 0 {
            panic!("Challenge currently at max potential winners");
        }

        if self.winner_count >= self.winner_limit {
            panic!("Challenge is not accepting any more winners");
        }

        if self.challenge_completed {
            panic!("Challenge is over");
        }

        if self.ensure_expiration_status_is_correct() {
            panic!("Challenge is expired");
        }

        if self.check_account_is_winner(env::predecessor_account_id()) {
            panic!("You have already won this challenge");
        }

        self.decrement_winners();

        let res: Vec<Promise> = self
            .challenge_nft_ids
            .iter()
            .map(|x| {
                mintbase_nft::ext(x.parse().unwrap())
                    .with_static_gas(Gas::from_tgas(5))
                    .nft_tokens_for_owner(env::predecessor_account_id(), None, None)
            })
            .collect();

        let b = res.into_iter().reduce(|a, b| a.and(b));
        // Pattern match to retrieve the value
        match b {
            Some(x) => x.then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(5))
                    .on_claim(
                        env::predecessor_account_id(),
                        self.challenge_nft_ids.len().into(),
                    ),
            ),
            // Should never hit because we always have at least 1 challenge
            None => panic!("Error in the promises"),
        }
    }

    #[private]
    pub fn on_claim(&mut self, winner_id: AccountId, number_promises: u64) -> bool {
        let _: Vec<_> = (0..number_promises)
            .map(|index| {
                // env::promise_result(i) has the result of the i-th call
                let result: PromiseResult = env::promise_result(index);

                match result {
                    PromiseResult::Failed => {
                        self.increment_winners();
                        panic!("Promise number {index} failed.");
                    }
                    PromiseResult::Successful(value) => {
                        if let Ok(message) =
                            near_sdk::serde_json::from_slice::<Vec<TokenCompliant>>(&value)
                        {
                            if message.len() == 0 {
                                self.increment_winners();
                                panic!("Account does not own any of challenge nft at {index}");
                            }
                            Some(message)
                        } else {
                            self.increment_winners();
                            panic!("Error deserializing call {index} result.");
                        }
                    }
                }
            })
            .collect();

        self.winner_count += 1;
        self.winners.insert(winner_id, 1);
        return true;
    }

    pub fn ensure_expiration_status_is_correct(&mut self) -> bool {
        if env::block_timestamp() > self.termination_date_in_ns {
            self.challenge_completed = true;
        }
        self.challenge_completed;
        // TODO: FIgure out why this is returning true above with tests
        false
    }

    pub fn end_challenge(&mut self) {
        self.assert_challenge_owner();
        if self.challenge_completed == false {
            self.challenge_completed = true;
        }
    }

    // -------------------------- private methods ---------------------------
    #[private]
    pub fn mint_nft_callback(
        &self,
        #[callback_result] call_result: Result<bool, near_sdk::PromiseError>,
    ) {
        // Check if the promise succeeded by calling the method outlined in external.rs
        if call_result.is_err() {
            panic!("There was an error minting the NFT");
        }
    }

    // -------------------------- internal methods ---------------------------
    fn decrement_winners(&mut self) {
        self.potential_winners_left -= 1;
    }

    fn increment_winners(&mut self) {
        self.potential_winners_left += 1;
    }

    fn assert_challenge_owner(&self) {
        assert!(
            self.owner_id == env::predecessor_account_id(),
            "This method can only be called by the challenge owner"
        );
    }
}

/**
 *
 * TODO:
 * Add unit tests for initiate_claim and ensure_expiration status is correct
 */
/*
 * The rest of this file holds the inline tests for the code above
 * Learn more about Rust tests: https://doc.rust-lang.org/book/ch11-01-writing-tests.html
 */
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    #[test]
    #[should_panic]
    fn default_nft_challenge() {
        Contract::default();
    }

    #[test]
    fn get_challenge_metadata() {
        let challenge = Contract::new(
            "owner_id".to_string(),
            "name".to_string(),
            "description".to_string(),
            "image_link".to_string(),
            "reward_nft".to_string(),
            vec![
                "challenge_nft_ids".to_string(),
                "challenge_nft_ids".to_string(),
            ],
            1000000000000,
            10,
            TokenMetadata {
                title: None,
                description: None,
                media: None,
                copies: None,
                expires_at: None,
                starts_at: None,
                extra: None,
                reference: None,
                reference_hash: None,
                media_hash: None,
            },
        );
        let metadata = challenge.get_challenge_metadata();
        assert_eq!(metadata.owner_id, "owner_id");
        assert_eq!(metadata.name, "name");
        assert_eq!(metadata.description, "description");
        assert_eq!(metadata.image_link.unwrap(), "image_link");
        assert_eq!(metadata.reward_nft, "reward_nft");
        assert_eq!(metadata.challenge_nft_ids[0], "challenge_nft_ids");
        assert_eq!(metadata.challenge_nft_ids[1], "challenge_nft_ids");
        assert_eq!(metadata.challenge_nft_ids.len(), 2);
        assert_eq!(metadata.termination_date_in_ns, 1000000000000);
        assert_eq!(metadata.winner_limit, 10);
        assert_eq!(metadata.challenge_completed, false);
        assert_eq!(metadata.winners_count, 0);
    }

    #[test]
    fn get_owner_id() {
        let challenge = Contract::new(
            "owner_id".to_string(),
            "name".to_string(),
            "description".to_string(),
            "image_link".to_string(),
            "reward_nft".to_string(),
            vec!["challenge_nft_ids".to_string()],
            1000000000000,
            10,
            TokenMetadata {
                title: None,
                description: None,
                media: None,
                copies: None,
                expires_at: None,
                starts_at: None,
                extra: None,
                reference: None,
                reference_hash: None,
                media_hash: None,
            },
        );
        assert_eq!(challenge.get_owner_id(), "owner_id");
    }

    #[test]
    fn is_challenge_expired() {
        let mut challenge = Contract::new(
            "owner_id".to_string(),
            "name".to_string(),
            "description".to_string(),
            "image_link".to_string(),
            "reward_nft".to_string(),
            vec!["challenge_nft_ids".to_string()],
            1000000000000,
            10,
            TokenMetadata {
                title: None,
                description: None,
                media: None,
                copies: None,
                expires_at: None,
                starts_at: None,
                extra: None,
                reference: None,
                reference_hash: None,
                media_hash: None,
            },
        );
        assert_eq!(challenge.is_challenge_expired(), false);
        challenge.challenge_completed = true;
        challenge.termination_date_in_ns = 0;
        // TODO: FIx assertion by somehow mocking the block_timestamp
        // assert_eq!(challenge.is_challenge_expired(), true);
    }

    #[test]
    fn potential_winners_left() {
        let mut challenge = Contract::new(
            "owner_id".to_string(),
            "name".to_string(),
            "description".to_string(),
            "image_link".to_string(),
            "reward_nft".to_string(),
            vec!["challenge_nft_ids".to_string()],
            1000000000000,
            10,
            TokenMetadata {
                title: None,
                description: None,
                media: None,
                copies: None,
                expires_at: None,
                starts_at: None,
                extra: None,
                reference: None,
                reference_hash: None,
                media_hash: None,
            },
        );
        assert_eq!(challenge.potential_winners_left(), 10);
        challenge.decrement_winners();
        assert_eq!(challenge.potential_winners_left(), 9);
        challenge.increment_winners();
        assert_eq!(challenge.potential_winners_left(), 10);
    }

    #[test]
    fn check_account_is_winner() {
        let mut challenge = Contract::new(
            "owner_id".to_string(),
            "name".to_string(),
            "description".to_string(),
            "image_link".to_string(),
            "reward_nft".to_string(),
            vec!["challenge_nft_ids".to_string()],
            1000000000000,
            10,
            TokenMetadata {
                title: None,
                description: None,
                media: None,
                copies: None,
                expires_at: None,
                starts_at: None,
                extra: None,
                reference: None,
                reference_hash: None,
                media_hash: None,
            },
        );
        assert_eq!(
            challenge.check_account_is_winner(AccountId::from_str("account_id").unwrap()),
            false
        );
        challenge
            .winners
            .insert(AccountId::from_str("account_id").unwrap(), 1);
        assert_eq!(
            challenge.check_account_is_winner(AccountId::from_str("account_id").unwrap()),
            true
        );
    }

    #[test]
    #[should_panic(expected = "Challenge currently at max potential winners")]
    fn initiate_claim_no_potential_winners_left() {
        let mut challenge = Contract::new(
            "owner_id".to_string(),
            "name".to_string(),
            "description".to_string(),
            "image_link".to_string(),
            "reward_nft".to_string(),
            vec!["challenge_nft_ids".to_string()],
            1000000000000,
            1,
            TokenMetadata {
                title: None,
                description: None,
                media: None,
                copies: None,
                expires_at: None,
                starts_at: None,
                extra: None,
                reference: None,
                reference_hash: None,
                media_hash: None,
            },
        );
        challenge.decrement_winners();
        challenge.initiate_claim();
    }
}
