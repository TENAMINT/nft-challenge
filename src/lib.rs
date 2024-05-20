// Find all our documentation at https://docs.near.org

use near_sdk::{env, log, near, store::Vector, Gas, PanicOnDefault, Promise, PromiseResult};
pub mod external;
pub use crate::external::*;
use tokio::{io::join, join};

// Define the contract structure
#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    owner_id: String,
    challenge_list: Vector<String>,
    // challenge_burn_list: Vector<bool>,
    creator_id: String,
    termination_date: u64,
    challenge_completed: bool,
    winner_limit: u64,
    winner_count: u64,
    potential_winners_left: u64,
    reward_nft: String,
    name: String,
}

// Implement the contract structure
#[near]
impl Contract {
    #[init]
    pub fn new(
        owner_id: String,
        name: String,
        challenge_nfts: std::vec::Vec<String>,
        termination_date: u64,
        winner_limit: u64,
        reward_nft: String,
    ) -> Self {
        let mut challenge_list = Vector::new(b"a");
        for challenge in challenge_nfts.iter() {
            challenge_list.push(challenge.clone());
        }

        Self {
            owner_id,
            name: name,
            challenge_list: challenge_list,
            creator_id: env::predecessor_account_id().to_string(),
            termination_date,
            challenge_completed: false,
            winner_limit,
            winner_count: 0,
            potential_winners_left: winner_limit,
            reward_nft,
        }
    }

    // -------------------------- view methods ---------------------------
    pub fn check_can_mint(&self, panic_on_not_minter: bool) -> Promise {
        let promise = mintbase_nft::ext(self.reward_nft.parse().unwrap())
            .with_static_gas(Gas::from_tgas(5))
            .check_is_minter(env::current_account_id());

        return promise.then(
            // Create a promise to callback query_greeting_callback
            Self::ext(env::current_account_id())
                .with_static_gas(Gas::from_tgas(5))
                .check_is_minter_callback(panic_on_not_minter),
        );
    }
    /// Show the current owner of this NFT Challenge
    pub fn get_owner_id(&self) -> String {
        self.owner_id.clone()
    }

    pub fn is_challenge_expired(&self) -> bool {
        self.challenge_completed
    }

    pub fn winners_count(&self) -> u64 {
        self.winner_count
    }

    pub fn potential_winners_left(&self) -> u64 {
        self.potential_winners_left
    }
    pub fn winners_limit(&self) -> u64 {
        self.winner_limit
    }

    pub fn reward_nft(&self) -> String {
        self.reward_nft.clone()
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn challenge_list(&self) -> Vec<String> {
        let mut challenge_list = Vec::new();
        for challenge in self.challenge_list.iter() {
            challenge_list.push(challenge.clone());
        }
        challenge_list
    }

    // -------------------------- change methods ---------------------------
    // Not SURE if this will update state immediately, should though at the end of a function call

    // NEED TO LOCK!!!! State is only written back at the end of the function call!
    //https://docs.near.org/sdk/rust/contract-structure/near-bindgen
    // possible DOS attack surface
    // lets just ignore all locking logic right now and focus on core functionality
    pub fn initiate_claim(&mut self) -> bool {
        if self.potential_winners_left == 0
            || self.winner_count >= self.winner_limit
            || self.challenge_completed
            || self.ensure_expiration_status_is_correct()
        {
            // Might have to move into call back so I can check if we're a minter
            // || self
            //     .check_is_minter_callback(panic_on_not_minter, call_result)
            //     .await?
            return false;
        }
        self.decrement_winners();
        let mut vec = Vec::new();
        vec.push(1);
        vec.push(2);

        assert_eq!(vec.len(), 2);
        assert_eq!(vec[0], 1);

        let mut prromise = mintbase_nft::ext(self.challenge_list[0].parse().unwrap())
            .with_static_gas(Gas::from_tgas(5))
            .check_is_minter(env::current_account_id());

        let res: Vec<Promise> = self
            .challenge_list
            .iter()
            .map(|x| {
                mintbase_nft::ext(x.parse().unwrap())
                    .with_static_gas(Gas::from_tgas(5))
                    .check_is_minter(env::current_account_id())
            })
            .collect();

        let b = res.into_iter().reduce(|a, b| a.and(b));
        // Pattern match to retrieve the value
        match b {
            // The division was valid
            Some(x) => x.then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(5))
                    .similar_contracts_callback(3),
            ),
            // The division was invalid
            None => panic!("Error in the promises"),
        };

        // for challenge in self.challenge_list.iter() {}
        // TODO: Check that challenges are completed
        let challenges_completed = false;
        if !challenges_completed {
            self.increment_winners();
            return false;
        }

        self.winner_count += 1;
        mintbase_nft::ext(self.reward_nft.parse().unwrap())
            .with_static_gas(Gas::from_tgas(5))
            .nft_batch_mint(
                env::predecessor_account_id(),
                TokenMetadata {
                    title: None,
                    description: None,
                    media: None,
                    media_hash: None,
                    copies: Some(1),
                    expires_at: None,
                    starts_at: None,
                    extra: None,
                    reference: None,
                    reference_hash: None,
                },
                1,
                None,
                None,
            );
        // check if mint was successful in call back. If it was not increment potential winners.
        return true;
    }

    #[private]
    pub fn similar_contracts_callback(&self, number_promises: u64) -> Vec<String> {
        let t: Vec<_> = (0..number_promises)
            .filter_map(|index| {
                // env::promise_result(i) has the result of the i-th call
                let result = env::promise_result(index);

                match result {
                    PromiseResult::Failed => {
                        log!(format!("Promise number {index} failed."));
                        None
                    }
                    PromiseResult::Successful(value) => {
                        if let Ok(message) = near_sdk::serde_json::from_slice::<String>(&value) {
                            log!(format!("Call {index} returned: {message}"));
                            Some(message)
                        } else {
                            log!(format!("Error deserializing call {index} result."));
                            None
                        }
                    }
                }
            })
            .collect();
        panic!("Not implemented yet");
    }

    pub fn ensure_expiration_status_is_correct(&mut self) -> bool {
        if env::block_timestamp() > self.termination_date {
            self.challenge_completed = true;
        }
        self.challenge_completed
    }

    pub fn end_challenge(&mut self) {
        self.assert_store_owner();
        if self.challenge_completed == false {
            self.challenge_completed = true;
        }
    }

    // -------------------------- private methods ---------------------------

    #[private]
    pub fn check_is_minter_callback(
        &self,
        panic_on_not_minter: bool,
        #[callback_result] call_result: Result<bool, near_sdk::PromiseError>,
    ) -> bool {
        // Check if the promise succeeded by calling the method outlined in external.rs
        if call_result.is_err() && panic_on_not_minter {
            panic!("There was an error contacting NFT Contract");
        } else if call_result.is_err() {
            return false;
        }
        if call_result.unwrap() == false {
            panic!("The NFT contract has not given us mint access");
        }
        true
    }

    // -------------------------- internal methods ---------------------------

    fn decrement_winners(&mut self) {
        self.potential_winners_left -= 1;
    }

    fn increment_winners(&mut self) {
        self.potential_winners_left += 1;
    }

    fn assert_store_owner(&self) {
        assert!(
            self.owner_id == env::predecessor_account_id(),
            "This method can only be called by the store owner"
        );
    }
}

/*
 * The rest of this file holds the inline tests for the code above
 * Learn more about Rust tests: https://doc.rust-lang.org/book/ch11-01-writing-tests.html
 */
// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[test]
//     fn get_default_greeting() {
//         let contract = Contract::default();
//         // this test did not call set_greeting so should return the default "Hello" greeting
//         assert_eq!(contract.get_greeting(), "Hello");
//     }

//     #[test]
//     fn set_then_get_greeting() {
//         let mut contract = Contract::default();
//         contract.set_greeting("howdy".to_string());
//         assert_eq!(contract.get_greeting(), "howdy");
//     }
// }
