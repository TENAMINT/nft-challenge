use std::process::{ExitCode, Termination};

use mockall::predicate::*;
use near_sdk::{
    env, log, near,
    serde::{Deserialize, Serialize},
    store::{LookupMap, Vector},
    AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseResult,
};
pub mod external;
pub use crate::external::*;

/**
 * TODO: Add burn functionality to burn the NFTs after the challenge is over,
 * current obstacle is how to do this without needed nft contracts to give us
 * burn permissions
 *
 * Can someone win a challenge multiple times? If so we need to burn challenge
 * nfts to prevent spam.
 *
 * If someone wins a challenge, can they transfer challenge nfts to another
 * account and win again?.
 *
 */

impl Termination for Contract {
    fn report(self) -> std::process::ExitCode {
        ExitCode::SUCCESS
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChallengeMetaData {
    // The owner of this NFT Challenge
    pub owner_id: String,
    // The name for this challenge.
    pub name: String,
    // Free-form description of this challenge.
    pub description: String,
    // URL to associated media, preferably to decentralized, content-addressed storage
    pub media_link: Option<String>,
    // The id of the reward NFT.
    pub reward_nft_id: String,
    // Metadata for the reward token NFT. Only necessary if we mint the nft.
    pub reward_nft_metadata: NFTTokenMetadata,
    // Ids of the challenge nfts that are part of this challenge.
    pub challenge_nft_ids: Vec<String>,
    // The expiration date of this challenge, expressed as a nano second timestamp.
    pub expiration_date_in_ns: u64,
    // Maximum number of winners for this challenge.
    pub winner_limit: u64,
    // Number of winners for this challenge.
    pub winners_count: u64,
    // Whether the challenge is completed or not.
    pub challenge_completed: bool,
}

// Define the contract structure
#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    // The owner of this NFT Challenge
    owner_id: String,
    // The creator of this NFT Challenge
    creator_id: String,
    // The name for this challenge.
    name: String,
    // Free-form description of this challenge.
    description: String,
    // URL to associated media, preferably to decentralized, content-addressed storage
    media_link: String,
    // The id of the reward NFT.
    reward_nft_id: String,
    // Metadata for the reward token NFT. Only necessary if we mint the nft.
    reward_nft_metadata: NFTTokenMetadata,
    // Ids of the challenge nfts that are part of this challenge.
    challenge_nft_ids: Vector<String>,
    // The expiration date of this challenge, expressed as a nano second timestamp.
    expiration_date_in_ns: u64,
    // Maximum number of winners for this challenge.
    winner_limit: u64,
    // Current number of winners for this challenge.
    winner_count: u64,
    // The list of winners for this challenge. This is a map and not a set
    // in case we want to let winners win multiple times.
    winners: LookupMap<AccountId, u64>,
    // The number of potential winners left for this challenge.
    potential_winners_left: u64,
    // Whether the challenge is completed or not.
    challenge_completed: bool,
}

// Implement the contract structure
#[near]
impl Contract {
    #[init]
    pub fn new(
        owner_id: String,
        name: String,
        description: String,
        media_link: String,
        reward_nft_id: String,
        _challenge_nft_ids: std::vec::Vec<String>,
        expiration_date_in_ns: u64,
        winner_limit: u64,
        reward_nft_metadata: NFTTokenMetadata,
    ) -> Self {
        let mut challenge_nft_ids = Vector::new(b"a");
        for challenge in _challenge_nft_ids.iter() {
            challenge_nft_ids.push(challenge.clone());
        }

        assert!(
            _challenge_nft_ids.len() > 0,
            "Challenge must have at least 1 challenge NFT"
        );
        Self {
            owner_id,
            creator_id: env::predecessor_account_id().to_string(),
            name,
            description,
            media_link,
            reward_nft_id,
            challenge_nft_ids,
            expiration_date_in_ns,
            winner_limit,
            challenge_completed: false,
            winner_count: 0,
            potential_winners_left: winner_limit,
            winners: LookupMap::new(b"z"),
            reward_nft_metadata,
        }
    }

    // -------------------------- view methods ---------------------------
    pub fn mint_nft(&self) -> Promise {
        assert!(
            self.is_account_winner(env::predecessor_account_id()),
            "You must win the challenge to mint the NFT"
        );
        let promise = mintbase_nft::ext(self.reward_nft_id.parse().unwrap())
            // TODO: Get better gas and storage fee estimates.
            .with_static_gas(Gas::from_tgas(5))
            .with_attached_deposit(NearToken::from_millinear(54))
            .nft_batch_mint(
                env::predecessor_account_id(),
                self.reward_nft_metadata.clone(),
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
            media_link: Some(self.media_link.clone()),
            reward_nft_id: self.reward_nft_id.clone(),
            challenge_nft_ids: challenge_list,
            expiration_date_in_ns: self.expiration_date_in_ns,
            winner_limit: self.winner_limit,
            challenge_completed: self.challenge_completed,
            winners_count: self.winner_count,
            reward_nft_metadata: self.reward_nft_metadata.clone(),
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
        env::block_timestamp() >= self.expiration_date_in_ns
    }

    pub fn potential_winners_left(&self) -> u64 {
        self.potential_winners_left
    }

    pub fn is_account_winner(&self, account_id: AccountId) -> bool {
        self.winners.contains_key(&account_id)
    }

    // -------------------------- change methods ---------------------------
    pub fn initiate_claim(&mut self) -> Promise {
        log!("max potential winners left {}", self.potential_winners_left);
        if self.potential_winners_left == 0 {
            panic!("Challenge currently at max potential winners");
        }

        if self.winner_count >= self.winner_limit {
            panic!("Challenge is not accepting any more winners");
        }

        if self.challenge_completed {
            panic!("Challenge is over");
        }

        if self.ensure_challenge_not_expired() {
            panic!("Challenge is expired");
        }

        if self.is_account_winner(env::predecessor_account_id()) {
            panic!("You have already won this challenge");
        }

        self.decrement_winners();

        let challenge_nft_ownership_promises: Vec<Promise> = self
            .challenge_nft_ids
            .iter()
            .map(|x| {
                mintbase_nft::ext(x.parse().unwrap())
                    .with_static_gas(Gas::from_tgas(5))
                    .nft_tokens_for_owner(env::predecessor_account_id(), None, None)
            })
            .collect();

        let compiled_promise = challenge_nft_ownership_promises
            .into_iter()
            .reduce(|a, b| a.and(b));
        // Pattern match to retrieve the value
        match compiled_promise {
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
        let res: Vec<bool> = (0..number_promises)
            .map(|index| {
                // env::promise_result(i) has the result of the i-th call
                let result: PromiseResult = env::promise_result(index);

                match result {
                    PromiseResult::Failed => false,
                    PromiseResult::Successful(value) => {
                        if let Ok(message) =
                            near_sdk::serde_json::from_slice::<Vec<TokenCompliant>>(&value)
                        {
                            message.len() != 0
                        } else {
                            false
                        }
                    }
                }
            })
            .collect();

        for i in 0..res.len() {
            if res[i] == false {
                self.increment_winners();
                log!(
                    "max potential winners before 33 {}",
                    self.potential_winners_left
                );
                log!("Account does not own any of challenge nfts at {}", i);
                return false;
            }
        }
        self.winner_count += 1;
        self.winners.insert(winner_id, 1);
        return true;
    }

    pub fn ensure_challenge_not_expired(&mut self) -> bool {
        if env::block_timestamp() > self.expiration_date_in_ns {
            self.challenge_completed = true;
        }
        self.challenge_completed
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
    fn new() -> Contract {
        Contract::new(
            "owner_id".to_string(),
            "name".to_string(),
            "description".to_string(),
            "media_link".to_string(),
            "reward_nft".to_string(),
            vec![
                "challenge_nft_ids".to_string(),
                "challenge_nft_ids".to_string(),
            ],
            1000000000000,
            1,
            NFTTokenMetadata {
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
        )
    }

    #[test]
    fn get_challenge_metadata() {
        let challenge = new();
        let metadata = challenge.get_challenge_metadata();
        assert_eq!(metadata.owner_id, "owner_id");
        assert_eq!(metadata.name, "name");
        assert_eq!(metadata.description, "description");
        assert_eq!(metadata.media_link.unwrap(), "media_link");
        assert_eq!(metadata.reward_nft_id, "reward_nft");
        assert_eq!(metadata.challenge_nft_ids[0], "challenge_nft_ids");
        assert_eq!(metadata.challenge_nft_ids[1], "challenge_nft_ids");
        assert_eq!(metadata.challenge_nft_ids.len(), 2);
        assert_eq!(metadata.expiration_date_in_ns, 1000000000000);
        assert_eq!(metadata.winner_limit, 1);
        assert_eq!(metadata.challenge_completed, false);
        assert_eq!(metadata.winners_count, 0);
    }

    #[test]
    fn get_owner_id() {
        let challenge = new();
        assert_eq!(challenge.get_owner_id(), "owner_id");
    }

    #[test]
    fn is_challenge_expired() {
        let mut challenge = new();
        assert_eq!(challenge.is_challenge_expired(), false);
        challenge.challenge_completed = true;
        challenge.expiration_date_in_ns = 0;
        // TODO: FIx assertion by somehow mocking the block_timestamp
        assert_eq!(challenge.is_challenge_expired(), true);
    }

    #[test]
    fn potential_winners_left() {
        let mut challenge = new();
        assert_eq!(challenge.potential_winners_left(), 1);
        challenge.decrement_winners();
        assert_eq!(challenge.potential_winners_left(), 0);
        challenge.increment_winners();
        assert_eq!(challenge.potential_winners_left(), 1);
    }

    #[test]
    fn is_account_winner() {
        let mut challenge = new();
        assert_eq!(
            challenge.is_account_winner(AccountId::from_str("account_id").unwrap()),
            false
        );
        challenge
            .winners
            .insert(AccountId::from_str("account_id").unwrap(), 1);
        assert_eq!(
            challenge.is_account_winner(AccountId::from_str("account_id").unwrap()),
            true
        );
    }

    #[test]
    #[should_panic(expected = "Challenge currently at max potential winners")]
    fn initiate_claim_no_potential_winners_left() {
        let mut challenge = new();
        challenge.decrement_winners();
        challenge.initiate_claim();
    }
}
