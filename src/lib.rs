use std::process::{ExitCode, Termination};

use mockall::predicate::*;
use near_sdk::{
    env,
    json_types::U64,
    log, near,
    serde::{Deserialize, Serialize},
    store::{LookupMap, LookupSet, Vector},
    AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseResult,
};
pub mod external;
pub use crate::external::*;

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
    // Whether to burn the challenge piece at the associated index when claiming.
    pub burn_challenge_piece_on_claim: Vec<bool>,
    // The expiration date of this challenge, expressed as a nano second timestamp.
    pub expiration_date_in_ns: u64,
    // Maximum number of winners for this challenge.
    pub winner_limit: u64,
    // Number of winners for this challenge.
    pub winners_count: u64,
    // Whether the challenge is completed or not.
    pub challenge_completed: bool,
    // Whether the creator of this challenge can update the completion status.
    creator_can_update: bool,
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
    // Whether to burn the challenge piece at the associated index when claiming.
    burn_challenge_piece_on_claim: Vector<bool>,
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
    // Whether the creator of this challenge can update the completion status.
    creator_can_update: bool,
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
        _burn_challenge_piece_on_claim: std::vec::Vec<bool>,
        expiration_date_in_ns: u64,
        winner_limit: u64,
        creator_can_update: bool,
        reward_nft_metadata: NFTTokenMetadata,
    ) -> Self {
        assert!(
            env::is_valid_account_id(owner_id.as_bytes()),
            "Owner's account ID is invalid",
        );
        assert_eq!(
            _challenge_nft_ids.len(),
            _burn_challenge_piece_on_claim.len(),
            "The challenge nft ids and burn challenge piece on claim must be the same length"
        );
        assert!(
            _challenge_nft_ids.len() > 0,
            "Challenge must have at least 1 challenge NFT"
        );
        let mut challenge_nft_ids_set = LookupSet::new(b"t");
        let mut challenge_nft_ids = Vector::new(b"a");
        let mut burn_challenge_piece_on_claim = Vector::new(b"c");
        for i in 0.._challenge_nft_ids.len() {
            if challenge_nft_ids_set.contains(&_challenge_nft_ids[i]) {
                panic!("Challenge NFT ids must be unique");
            }
            challenge_nft_ids.push(_challenge_nft_ids[i].clone());
            challenge_nft_ids_set.insert(&_challenge_nft_ids[i]);
            burn_challenge_piece_on_claim.push(_burn_challenge_piece_on_claim[i]);
        }

        Self {
            owner_id,
            creator_id: env::predecessor_account_id().to_string(),
            name,
            description,
            media_link,
            reward_nft_id,
            challenge_nft_ids,
            burn_challenge_piece_on_claim,
            expiration_date_in_ns,
            winner_limit,
            challenge_completed: false,
            winner_count: 0,
            potential_winners_left: winner_limit,
            winners: LookupMap::new(b"z"),
            reward_nft_metadata,
            creator_can_update,
        }
    }

    // -------------------------- view methods ---------------------------

    pub fn get_challenge_metadata(&self) -> ChallengeMetaData {
        let mut challenge_list = Vec::new();
        let mut challenge_burn_list = Vec::new();
        for i in 0..self.challenge_nft_ids.len() {
            challenge_list.push(self.challenge_nft_ids[i].clone());
            challenge_burn_list.push(self.burn_challenge_piece_on_claim[i]);
        }
        ChallengeMetaData {
            owner_id: self.owner_id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            media_link: Some(self.media_link.clone()),
            reward_nft_id: self.reward_nft_id.clone(),
            challenge_nft_ids: challenge_list,
            burn_challenge_piece_on_claim: challenge_burn_list,
            expiration_date_in_ns: self.expiration_date_in_ns,
            winner_limit: self.winner_limit,
            challenge_completed: self.challenge_completed,
            winners_count: self.winner_count,
            reward_nft_metadata: self.reward_nft_metadata.clone(),
            creator_can_update: self.creator_can_update,
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

    pub fn is_challenge_complete(&self) -> bool {
        self.challenge_completed
    }

    // -------------------------- change methods ---------------------------

    #[payable]
    pub fn mint_nft(&mut self) -> Promise {
        assert!(
            self.is_account_winner(env::predecessor_account_id()),
            "You must win the challenge to mint the NFT"
        );
        assert!(
            env::attached_deposit().as_millinear() >= 54,
            "To cover minting fees, you need to attach at least {} millinear to this transaction.",
            // TODO: Figure out more accurate deposit
            54
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

    #[payable]
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
    pub fn on_claim(&mut self, winner_id: AccountId, number_promises: u64) -> Option<Promise> {
        let mut token_ids_to_burn: Vec<U64> = vec![];
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
                            if message.len() != 0 {
                                if self.burn_challenge_piece_on_claim[index as u32] {
                                    token_ids_to_burn
                                        .push(U64(message[0].token_id.parse().unwrap()));
                                }
                                true
                            } else {
                                false
                            }
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
                log!("Account does not own any of challenge nfts at {}", i);
                return None;
            }
        }
        if token_ids_to_burn.len() == 0 {
            self.winner_count += 1;
            self.winners.insert(winner_id, 1);
            return None;
        }
        Some(self.have_approvals_for_transfers(winner_id,token_ids_to_burn))
    }

    #[payable]
    #[private]
    pub fn have_approvals_for_transfers(&mut self, winner_id: AccountId,token_ids: Vec<U64>) -> Promise {
        let mut is_approved_promises: Vec<Promise> = vec![];
   
        for i in 0..self.burn_challenge_piece_on_claim.len() {
           
            is_approved_promises.push(
                mintbase_nft::ext(
                    self.challenge_nft_ids[i.try_into().unwrap()]
                        .parse()
                        .unwrap(),
                )
                .with_static_gas(Gas::from_tgas(5))
                .nft_approval_id(token_ids[i as usize], env::current_account_id()),
            );
        }
        let compiled_promise = is_approved_promises.into_iter().reduce(|a, b| a.and(b));
        if compiled_promise.is_none() {
            panic!("No nfts to burn. Should not have reached here.");
        } else {
            compiled_promise.unwrap().then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(5))
                    .on_approval_check(winner_id,token_ids),
            )
        }
    }

    #[payable]
    #[private]
    pub fn on_approval_check(&mut self, winner_id: AccountId,token_ids: Vec<U64>) -> Promise {
        let approvals : Vec<Option<u64>> = (0..token_ids.len())
            .map(|index| {
                // env::promise_result(i) has the result of the i-th call
                let result: PromiseResult = env::promise_result(index as u64);
               
                match result {
                    PromiseResult::Failed => {
                        log!(
                            "You must grant transfer approval for the challenge NFT at index {} for us to burn it",
                            index
                        );
                        None
                    },
                    PromiseResult::Successful(value) => {
                        if let Ok(message) =
                            near_sdk::serde_json::from_slice::<u64>(&value)
                        {
                           Some(message)
                        } else {
                            log!("You must grant transfer approval for the challenge NFT at index {} for us to burn it",index);
                            None
                        }
                    }
                }
            })
            .collect();
        for i in 0..approvals.len() {
            if approvals[i] == None {
                self.increment_winners();
                return Promise::new(env::current_account_id()).as_return();
            }
        }
        let mut transfer_promises: Vec<Promise> = vec![];
        for i in 0..self.burn_challenge_piece_on_claim.len() {
            transfer_promises.push(
                mintbase_nft::ext(
                    self.challenge_nft_ids[i.try_into().unwrap()]
                        .parse()
                        .unwrap(),
                )
                .with_static_gas(Gas::from_tgas(5))
                .with_attached_deposit(NearToken::from_yoctonear(1))
                .nft_transfer(
                    env::current_account_id(),
                    token_ids[i as usize],
                    approvals[i as usize].unwrap(),
                    None,
                ),
            );
        }
        let compiled_promise = transfer_promises.into_iter().reduce(|a, b| a.and(b));
        if compiled_promise.is_none() {
            panic!("No nfts to burn. Should not have reached here.");
        } else {
            compiled_promise.unwrap().then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(5))
                    .burn_nfts(winner_id,token_ids),
            )
        }
    }

    #[payable]
    #[private]
    pub fn burn_nfts(&mut self,winner_id: AccountId, token_ids: Vec<U64>) -> Promise {
        //TODO: check if transfers were completed successfully. If not, return the tokens to the user
        let mut burn_promises: Vec<Promise> = vec![];
        for i in 0..self.burn_challenge_piece_on_claim.len() {
            burn_promises.push(
                mintbase_nft::ext(
                    self.challenge_nft_ids[i.try_into().unwrap()]
                        .parse()
                        .unwrap(),
                )
                .with_static_gas(Gas::from_tgas(5))
                .with_attached_deposit(NearToken::from_yoctonear(1))
                .nft_batch_burn(vec![token_ids[i as usize].clone()]),
            );
        }
        let burn_count = burn_promises.len() as u64; // Convert usize to u64
        let compiled_promise = burn_promises.into_iter().reduce(|a, b| a.and(b));

        if compiled_promise.is_none() {
            panic!("No nfts to burn. Should not have reached here.");
        } else {
            compiled_promise.unwrap().then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(5))
                    .on_burn_nfts(winner_id,burn_count),
            )
        }
    }

    #[private]
    pub fn on_burn_nfts(&mut self,winner_id: AccountId, number_promises: u64) -> bool {
        let results: Vec<bool> = (0..number_promises)
            .map(|index| {
                // env::promise_result(i) has the result of the i-th call
                let result: PromiseResult = env::promise_result(index);
                if result == PromiseResult::Failed {}
                match result {
                    PromiseResult::Failed => {
                        log!(
                            "There was an error burning the challenge NFT at index {}",
                            index
                        );
                        false
                    }
                    PromiseResult::Successful(_) => {
                        log!("NFT burned successfully at index {}", index);
                        true
                    }
                }
            })
            .collect();
        for i in 0..results.len() {
            if results[i] == false {
                self.increment_winners();
                return false;
            }
        }
        self.winner_count += 1;
        self.winners.insert(winner_id, 1);
        true
    }

    pub fn update_challenge_completion_status(&mut self, is_complete: bool) {
        self.assert_challenge_owner();
        if self.creator_can_update {
            self.challenge_completed = is_complete;
        } else {
            panic!("The creator cannot update the completion status of this challenge");
        }
    }

    pub fn ensure_challenge_not_expired(&mut self) -> bool {
        if env::block_timestamp() > self.expiration_date_in_ns {
            self.challenge_completed = true;
        }
        self.challenge_completed
    }

    // -------------------------- private methods ---------------------------
    #[private]
    pub fn mint_nft_callback(
        &self,
        #[callback_result] call_result: Result<(), near_sdk::PromiseError>,
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
                "challenge_nft_id1".to_string(),
                "challenge_nft_id2".to_string(),
            ],
            vec![true, false],
            1000000000000,
            1,
            true,
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
        assert_eq!(metadata.challenge_nft_ids[0], "challenge_nft_id1");
        assert_eq!(metadata.challenge_nft_ids[1], "challenge_nft_id2");
        assert_eq!(metadata.burn_challenge_piece_on_claim[0], true);
        assert_eq!(metadata.burn_challenge_piece_on_claim[1], false);
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
