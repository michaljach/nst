//! # Non Speculative Token (NST) - UBI Pallet
//!
//! A burn-only Universal Basic Income token designed to prevent speculation and trading.
//!
//! ## Key Features
//!
//! - **Daily UBI Claims**: Every wallet can claim 100 tokens/day (up to 3 days backlog)
//! - **Burn-Only Spending**: Tokens cannot be transferred, only burned with a named recipient
//! - **Automatic Expiration**: Tokens expire after 7 days if not used
//! - **Reputation Tracking**: On-chain tracking of burns sent/received for social reputation
//! - **Open Participation**: Any wallet can participate (sybil-resistant via expiration)
//!
//! ## Why Burn-Only?
//!
//! Traditional cryptocurrencies allow transfers, enabling speculation and trading.
//! By making tokens burn-only:
//! - Exchanges cannot operate (nothing to sell after receiving)
//! - No accumulation possible (tokens expire)
//! - Value comes from utility + reputation, not speculation
//!
//! ## Token Lifecycle
//!
//! ```text
//! CLAIM (100/day) → HOLD (max 7 days) → BURN (to recipient) → DESTROYED
//!                         ↓
//!                    EXPIRE (if unused)
//! ```
//!
//! ## Reputation System
//!
//! When tokens are burned, both parties' reputation is updated:
//! - Sender: burns_sent_count++, burns_sent_volume += amount
//! - Recipient: burns_received_count++, burns_received_volume += amount
//!
//! This creates a view-only reputation system with no penalties.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::{CheckedAdd, CheckedSub, Saturating, Zero};

/// A batch of tokens with an expiration block
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct TokenBatch<BlockNumber> {
    /// Amount of tokens in this batch
    pub amount: u128,
    /// Block number when these tokens expire
    pub expires_at: BlockNumber,
}

/// Reputation data for an account
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct Reputation<BlockNumber> {
    /// Number of burn transactions sent
    pub burns_sent_count: u64,
    /// Total volume of tokens burned (sent)
    pub burns_sent_volume: u128,
    /// Number of burn transactions received
    pub burns_received_count: u64,
    /// Total volume of tokens burned to this account
    pub burns_received_volume: u128,
    /// Block number of first activity (claim or burn)
    pub first_activity: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// Maximum number of token batches per account
    pub const MAX_BATCHES: u32 = 10;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configuration trait for the UBI token pallet
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Amount of tokens distributed per claim period (daily UBI)
        #[pallet::constant]
        type UbiAmount: Get<u128>;

        /// Number of blocks in one claim period (e.g., 1 day worth of blocks)
        #[pallet::constant]
        type ClaimPeriodBlocks: Get<BlockNumberFor<Self>>;

        /// Number of blocks until tokens expire (e.g., 7 days worth of blocks)
        #[pallet::constant]
        type ExpirationBlocks: Get<BlockNumberFor<Self>>;

        /// Maximum number of claim periods that can be claimed as backlog
        #[pallet::constant]
        type MaxBacklogPeriods: Get<u32>;
    }

    /// Token balances stored as batches with expiration
    #[pallet::storage]
    #[pallet::getter(fn balances)]
    pub type Balances<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<TokenBatch<BlockNumberFor<T>>, ConstU32<MAX_BATCHES>>,
        ValueQuery,
    >;

    /// Block number of last claim for each account
    #[pallet::storage]
    #[pallet::getter(fn last_claim)]
    pub type LastClaim<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BlockNumberFor<T>, OptionQuery>;

    /// Reputation data for each account
    #[pallet::storage]
    #[pallet::getter(fn reputation)]
    pub type ReputationStore<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Reputation<BlockNumberFor<T>>, ValueQuery>;

    /// Total tokens currently in circulation (not expired)
    #[pallet::storage]
    #[pallet::getter(fn total_supply)]
    pub type TotalSupply<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// Events emitted by this pallet
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Tokens were claimed from UBI
        Claimed {
            who: T::AccountId,
            amount: u128,
            periods: u32,
            expires_at: BlockNumberFor<T>,
        },
        /// Tokens were burned (payment made)
        Burned {
            from: T::AccountId,
            to: T::AccountId,
            amount: u128,
        },
        /// Tokens expired and were removed
        Expired {
            who: T::AccountId,
            amount: u128,
        },
    }

    /// Errors that can occur in this pallet
    #[pallet::error]
    pub enum Error<T> {
        /// No claimable periods available (must wait for next period)
        NothingToClaim,
        /// Insufficient balance for burn operation
        InsufficientBalance,
        /// Cannot burn to yourself
        CannotBurnToSelf,
        /// Amount must be greater than zero
        AmountMustBePositive,
        /// Too many token batches (should not happen with lazy cleanup)
        TooManyBatches,
        /// Arithmetic overflow
        Overflow,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim your daily UBI tokens
        ///
        /// Each wallet can claim once per period (default: 1 day).
        /// If you miss days, you can claim up to 3 periods of backlog.
        /// Claimed tokens expire after 7 days if not used.
        ///
        /// # Errors
        /// - `NothingToClaim` if you've already claimed this period and have no backlog
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(3, 3))]
        pub fn claim(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let current_block = frame_system::Pallet::<T>::block_number();
            let claim_period = T::ClaimPeriodBlocks::get();
            let ubi_amount = T::UbiAmount::get();
            let max_backlog = T::MaxBacklogPeriods::get();

            // Calculate claimable periods
            let claimable_periods = Self::calculate_claimable_periods(&who, current_block);
            ensure!(claimable_periods > 0, Error::<T>::NothingToClaim);

            // Cap at max backlog
            let periods_to_claim = claimable_periods.min(max_backlog);
            let amount_to_claim = ubi_amount.saturating_mul(periods_to_claim as u128);

            // Clean up expired batches first
            let expired = Self::cleanup_expired_batches(&who, current_block);
            if expired > 0 {
                Self::deposit_event(Event::Expired {
                    who: who.clone(),
                    amount: expired,
                });
            }

            // Calculate expiration for new batch
            let expires_at = current_block.saturating_add(T::ExpirationBlocks::get());

            // Create new batch
            let new_batch = TokenBatch {
                amount: amount_to_claim,
                expires_at,
            };

            // Add to balances
            Balances::<T>::try_mutate(&who, |batches| -> DispatchResult {
                // Try to merge with existing batch that has same expiration
                let merged = batches.iter_mut().any(|b| {
                    if b.expires_at == expires_at {
                        b.amount = b.amount.saturating_add(amount_to_claim);
                        true
                    } else {
                        false
                    }
                });

                if !merged {
                    batches
                        .try_push(new_batch)
                        .map_err(|_| Error::<T>::TooManyBatches)?;
                }
                Ok(())
            })?;

            // Update last claim block
            LastClaim::<T>::insert(&who, current_block);

            // Update total supply
            TotalSupply::<T>::mutate(|supply| {
                *supply = supply.saturating_add(amount_to_claim);
            });

            // Update first activity if this is the first time
            ReputationStore::<T>::mutate(&who, |rep| {
                if rep.first_activity == Zero::zero() {
                    rep.first_activity = current_block;
                }
            });

            Self::deposit_event(Event::Claimed {
                who,
                amount: amount_to_claim,
                periods: periods_to_claim,
                expires_at,
            });

            Ok(())
        }

        /// Burn tokens to a recipient (make a payment)
        ///
        /// This is the ONLY way to "spend" tokens. The recipient does not
        /// receive any tokens - they only see the burn event. This prevents
        /// trading and speculation.
        ///
        /// Both parties' reputation is updated:
        /// - Sender: burns_sent increases
        /// - Recipient: burns_received increases
        ///
        /// # Arguments
        /// - `to`: The recipient address (for reputation tracking and event)
        /// - `amount`: Number of tokens to burn
        ///
        /// # Errors
        /// - `CannotBurnToSelf` if trying to burn to your own address
        /// - `AmountMustBePositive` if amount is zero
        /// - `InsufficientBalance` if you don't have enough tokens
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(4, 4))]
        pub fn burn(origin: OriginFor<T>, to: T::AccountId, amount: u128) -> DispatchResult {
            let from = ensure_signed(origin)?;

            // Validation
            ensure!(from != to, Error::<T>::CannotBurnToSelf);
            ensure!(amount > 0, Error::<T>::AmountMustBePositive);

            let current_block = frame_system::Pallet::<T>::block_number();

            // Clean up expired batches first
            let expired = Self::cleanup_expired_batches(&from, current_block);
            if expired > 0 {
                Self::deposit_event(Event::Expired {
                    who: from.clone(),
                    amount: expired,
                });
            }

            // Check balance and burn using FIFO
            Self::burn_fifo(&from, amount, current_block)?;

            // Update total supply
            TotalSupply::<T>::mutate(|supply| {
                *supply = supply.saturating_sub(amount);
            });

            // Update sender reputation
            ReputationStore::<T>::mutate(&from, |rep| {
                rep.burns_sent_count = rep.burns_sent_count.saturating_add(1);
                rep.burns_sent_volume = rep.burns_sent_volume.saturating_add(amount);
                if rep.first_activity == Zero::zero() {
                    rep.first_activity = current_block;
                }
            });

            // Update recipient reputation
            ReputationStore::<T>::mutate(&to, |rep| {
                rep.burns_received_count = rep.burns_received_count.saturating_add(1);
                rep.burns_received_volume = rep.burns_received_volume.saturating_add(amount);
                if rep.first_activity == Zero::zero() {
                    rep.first_activity = current_block;
                }
            });

            Self::deposit_event(Event::Burned { from, to, amount });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Calculate how many periods the account can claim
        fn calculate_claimable_periods(
            who: &T::AccountId,
            current_block: BlockNumberFor<T>,
        ) -> u32 {
            let claim_period = T::ClaimPeriodBlocks::get();

            match LastClaim::<T>::get(who) {
                None => {
                    // Never claimed before - can claim 1 period
                    1
                }
                Some(last_claim_block) => {
                    // Calculate periods since last claim
                    let blocks_since = current_block.saturating_sub(last_claim_block);

                    // Convert to periods (integer division)
                    let periods_since: u32 = (blocks_since / claim_period)
                        .try_into()
                        .unwrap_or(u32::MAX);

                    periods_since
                }
            }
        }

        /// Remove expired batches and return total expired amount
        fn cleanup_expired_batches(
            who: &T::AccountId,
            current_block: BlockNumberFor<T>,
        ) -> u128 {
            let mut expired_amount: u128 = 0;

            Balances::<T>::mutate(who, |batches| {
                let mut i = 0;
                while i < batches.len() {
                    if batches[i].expires_at <= current_block {
                        expired_amount = expired_amount.saturating_add(batches[i].amount);
                        batches.remove(i);
                    } else {
                        i += 1;
                    }
                }
            });

            // Update total supply for expired tokens
            if expired_amount > 0 {
                TotalSupply::<T>::mutate(|supply| {
                    *supply = supply.saturating_sub(expired_amount);
                });
            }

            expired_amount
        }

        /// Burn tokens using FIFO (oldest batches first)
        fn burn_fifo(
            who: &T::AccountId,
            mut amount: u128,
            current_block: BlockNumberFor<T>,
        ) -> DispatchResult {
            Balances::<T>::try_mutate(who, |batches| -> DispatchResult {
                // Sort by expiration (oldest first) for FIFO
                batches.sort_by(|a, b| a.expires_at.cmp(&b.expires_at));

                let mut remaining = amount;

                for batch in batches.iter_mut() {
                    // Skip expired batches (should be cleaned up, but just in case)
                    if batch.expires_at <= current_block {
                        continue;
                    }

                    if batch.amount >= remaining {
                        batch.amount = batch.amount.saturating_sub(remaining);
                        remaining = 0;
                        break;
                    } else {
                        remaining = remaining.saturating_sub(batch.amount);
                        batch.amount = 0;
                    }
                }

                // Remove empty batches
                batches.retain(|b| b.amount > 0);

                ensure!(remaining == 0, Error::<T>::InsufficientBalance);
                Ok(())
            })
        }

        /// Get the spendable balance (non-expired tokens) for an account
        pub fn spendable_balance(who: &T::AccountId) -> u128 {
            let current_block = frame_system::Pallet::<T>::block_number();
            let batches = Balances::<T>::get(who);

            batches
                .iter()
                .filter(|b| b.expires_at > current_block)
                .map(|b| b.amount)
                .fold(0u128, |acc, x| acc.saturating_add(x))
        }

        /// Get the total balance including expired (for informational purposes)
        pub fn total_balance(who: &T::AccountId) -> u128 {
            let batches = Balances::<T>::get(who);
            batches
                .iter()
                .map(|b| b.amount)
                .fold(0u128, |acc, x| acc.saturating_add(x))
        }

        /// Check if an account can claim UBI now
        pub fn can_claim(who: &T::AccountId) -> bool {
            let current_block = frame_system::Pallet::<T>::block_number();
            Self::calculate_claimable_periods(who, current_block) > 0
        }

        /// Get the number of claimable periods for an account
        pub fn claimable_periods(who: &T::AccountId) -> u32 {
            let current_block = frame_system::Pallet::<T>::block_number();
            let periods = Self::calculate_claimable_periods(who, current_block);
            periods.min(T::MaxBacklogPeriods::get())
        }

        /// Get the claimable amount for an account
        pub fn claimable_amount(who: &T::AccountId) -> u128 {
            let periods = Self::claimable_periods(who);
            T::UbiAmount::get().saturating_mul(periods as u128)
        }
    }
}
