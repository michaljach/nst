//! # Non Speculative Token (NST) - UBI Pallet
//!
//! A burn-only Universal Basic Income token designed to prevent speculation and trading.
//!
//! ## Key Features
//!
//! - **Daily UBI Claims**: Every wallet can claim 100 tokens/day (up to 3 days backlog)
//! - **Burn-Only Spending**: Tokens cannot be transferred, only burned with a named recipient
//! - **Automatic Expiration**: Tokens expire after 7 days if not used
//! - **Anti-Bot Reputation System**: Sophisticated reputation tracking resistant to farming
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
//! ## Enhanced Reputation System
//!
//! The reputation score is calculated from multiple factors:
//!
//! ```text
//! Score = (unique_recipients × 50) + (burns_sent × 1) + (weighted_received × 2) + streak_bonus
//! ```
//!
//! ### Anti-Bot Mechanisms:
//!
//! 1. **Sender Weight**: Burns from high-rep senders count more (0.5x to 2.0x)
//!    - New accounts have low weight, so bot farms can't bootstrap easily
//!    - Legitimate users receiving burns from established users grow faster
//!
//! 2. **Unique Recipients**: Only first burn to each recipient earns breadth bonus
//!    - Encourages spreading engagement across the community
//!    - Bot rings burning to same addresses don't accumulate bonus
//!
//! 3. **Claim Streak**: Rewards consistent daily claiming (10 points/day, max 500)
//!    - 2-day grace period before streak resets
//!    - Encourages regular participation
//!
//! 4. **Reputation Decay**: 5% decay per claim period
//!    - Inactive users' reputation slowly decreases
//!    - Must stay active to maintain high reputation

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
use sp_runtime::traits::{Saturating, Zero};
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction};

/// A batch of tokens with an expiration block
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct TokenBatch<BlockNumber> {
    /// Amount of tokens in this batch
    pub amount: u128,
    /// Block number when these tokens expire
    pub expires_at: BlockNumber,
}

/// Reputation data for an account
/// 
/// Reputation score is calculated as:
/// - unique_recipients_count × 50 (breadth of engagement)
/// - burns_sent_volume × 1 (giving to others)
/// - weighted_received × 2 (recognition from others, weighted by sender reputation)
/// - claim_streak × 10 (consistency bonus, capped at 500)
/// 
/// On each claim, reputation decays by 5% to encourage continued activity.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct Reputation<BlockNumber> {
    /// Number of burn transactions sent
    pub burns_sent_count: u64,
    /// Total volume of tokens burned (sent)
    pub burns_sent_volume: u128,
    /// Number of burn transactions received
    pub burns_received_count: u64,
    /// Total volume of tokens burned to this account (raw, unweighted)
    pub burns_received_volume: u128,
    /// Block number of first activity (claim or burn)
    pub first_activity: BlockNumber,
    
    // === New fields for enhanced reputation ===
    
    /// Weighted burns received (weighted by sender's reputation at time of burn)
    pub weighted_received: u128,
    /// Number of unique recipients this account has burned to
    pub unique_recipients_count: u32,
    /// Current claim streak (consecutive periods claimed)
    pub claim_streak: u32,
    /// Last claim period number (for streak tracking)
    pub last_claim_period: u64,
    /// Cached reputation score (updated on claim/burn)
    pub score: u128,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// Maximum number of token batches per account
    pub const MAX_BATCHES: u32 = 10;
    
    /// Maximum unique recipients to track per account
    pub const MAX_UNIQUE_RECIPIENTS: u32 = 1000;
    
    // Reputation calculation constants (using fixed-point math with 1000 = 1.0)
    /// Minimum sender weight (0.5 = 500/1000)
    pub const MIN_SENDER_WEIGHT: u128 = 500;
    /// Maximum sender weight (2.0 = 2000/1000)
    pub const MAX_SENDER_WEIGHT: u128 = 2000;
    /// Decay factor per claim (95% = 950/1000, i.e., 5% decay)
    pub const DECAY_FACTOR: u128 = 950;
    /// Reputation points per unique recipient
    pub const POINTS_PER_UNIQUE_RECIPIENT: u128 = 50;
    /// Reputation points per streak day (capped at 50 days = 500 points)
    pub const POINTS_PER_STREAK_DAY: u128 = 10;
    /// Maximum streak bonus
    pub const MAX_STREAK_BONUS: u128 = 500;
    /// Multiplier for weighted received in score (2x)
    pub const WEIGHTED_RECEIVED_MULTIPLIER: u128 = 2;
    /// Grace period for streak (can miss up to 2 periods)
    pub const STREAK_GRACE_PERIODS: u64 = 2;

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

    /// Track unique recipients for each sender (for reputation breadth bonus)
    /// Uses double map: sender -> recipient -> bool (exists)
    #[pallet::storage]
    pub type UniqueRecipients<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,  // sender
        Blake2_128Concat,
        T::AccountId,  // recipient
        bool,
        ValueQuery,
    >;

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
        /// Claim your daily UBI tokens (UNSIGNED - no gas fees!)
        ///
        /// Each wallet can claim once per period (default: 1 day).
        /// If you miss days, you can claim up to 3 periods of backlog.
        /// Claimed tokens expire after 7 days if not used.
        ///
        /// This is an UNSIGNED transaction - anyone can submit it without paying fees.
        /// The `account` parameter specifies who receives the UBI.
        ///
        /// # Errors
        /// - `NothingToClaim` if you've already claimed this period and have no backlog
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(3, 3))]
        pub fn claim(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
            ensure_none(origin)?;

            let who = account;
            let current_block = frame_system::Pallet::<T>::block_number();
            let _claim_period = T::ClaimPeriodBlocks::get();
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

            // Update reputation: decay, streak, and recalculate score
            let current_period = Self::block_to_period(current_block);
            ReputationStore::<T>::mutate(&who, |rep| {
                // Set first activity if this is the first time
                if rep.first_activity == Zero::zero() {
                    rep.first_activity = current_block;
                }
                
                // Apply 5% decay to current score
                rep.score = Self::apply_decay(rep.score);
                
                // Update claim streak (handles grace period logic)
                Self::update_streak(rep, current_period);
                
                // Recalculate full score from components
                rep.score = Self::recalculate_score(rep);
            });

            Self::deposit_event(Event::Claimed {
                who,
                amount: amount_to_claim,
                periods: periods_to_claim,
                expires_at,
            });

            Ok(())
        }

        /// Burn tokens to a recipient (UNSIGNED - no gas fees!)
        ///
        /// This is the ONLY way to "spend" tokens. The recipient does not
        /// receive any tokens - they only see the burn event. This prevents
        /// trading and speculation.
        ///
        /// Both parties' reputation is updated:
        /// - Sender: burns_sent increases
        /// - Recipient: burns_received increases
        ///
        /// This is an UNSIGNED transaction - anyone can submit it without paying fees.
        /// The `from` parameter specifies who is burning tokens.
        ///
        /// # Arguments
        /// - `from`: The sender address (who is burning tokens)
        /// - `to`: The recipient address (for reputation tracking and event)
        /// - `amount`: Number of tokens to burn
        ///
        /// # Errors
        /// - `CannotBurnToSelf` if trying to burn to your own address
        /// - `AmountMustBePositive` if amount is zero
        /// - `InsufficientBalance` if you don't have enough tokens
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(6, 6))]
        pub fn burn(origin: OriginFor<T>, from: T::AccountId, to: T::AccountId, amount: u128) -> DispatchResult {
            ensure_none(origin)?;

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

            // Get sender's current reputation score for weighting
            let sender_score = ReputationStore::<T>::get(&from).score;
            let sender_weight = Self::calculate_sender_weight(sender_score);
            
            // Calculate weighted amount: amount * weight / 1000
            let weighted_amount = amount.saturating_mul(sender_weight) / 1000;

            // Check if this is a new unique recipient for the sender
            let is_new_recipient = !UniqueRecipients::<T>::get(&from, &to);
            if is_new_recipient {
                UniqueRecipients::<T>::insert(&from, &to, true);
            }

            // Update sender reputation
            ReputationStore::<T>::mutate(&from, |rep| {
                rep.burns_sent_count = rep.burns_sent_count.saturating_add(1);
                rep.burns_sent_volume = rep.burns_sent_volume.saturating_add(amount);
                
                // Track unique recipients
                if is_new_recipient {
                    rep.unique_recipients_count = rep.unique_recipients_count.saturating_add(1);
                }
                
                if rep.first_activity == Zero::zero() {
                    rep.first_activity = current_block;
                }
                
                // Recalculate sender's score
                rep.score = Self::recalculate_score(rep);
            });

            // Update recipient reputation
            ReputationStore::<T>::mutate(&to, |rep| {
                rep.burns_received_count = rep.burns_received_count.saturating_add(1);
                rep.burns_received_volume = rep.burns_received_volume.saturating_add(amount);
                
                // Add weighted received (weighted by sender's reputation)
                rep.weighted_received = rep.weighted_received.saturating_add(weighted_amount);
                
                if rep.first_activity == Zero::zero() {
                    rep.first_activity = current_block;
                }
                
                // Recalculate recipient's score
                rep.score = Self::recalculate_score(rep);
            });

            Self::deposit_event(Event::Burned { from, to, amount });

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::claim { account } => {
                    // Validate that the account can actually claim
                    let current_block = frame_system::Pallet::<T>::block_number();
                    let claimable = Self::calculate_claimable_periods(account, current_block);
                    
                    if claimable == 0 {
                        return InvalidTransaction::Custom(1).into();
                    }
                    
                    ValidTransaction::with_tag_prefix("UbiClaim")
                        .and_provides((account, current_block / T::ClaimPeriodBlocks::get()))
                        .longevity(5)
                        .propagate(true)
                        .build()
                }
                Call::burn { from, to, amount } => {
                    // Basic validation
                    if from == to {
                        return InvalidTransaction::Custom(2).into();
                    }
                    if *amount == 0 {
                        return InvalidTransaction::Custom(3).into();
                    }
                    
                    // Check balance
                    let balance = Self::spendable_balance(from);
                    if balance < *amount {
                        return InvalidTransaction::Custom(4).into();
                    }
                    
                    ValidTransaction::with_tag_prefix("UbiBurn")
                        .and_provides((from, frame_system::Pallet::<T>::block_number()))
                        .longevity(5)
                        .propagate(true)
                        .build()
                }
                _ => InvalidTransaction::Call.into(),
            }
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
            amount: u128,
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

        // === New reputation system helpers ===

        /// Calculate sender weight based on their reputation score
        /// Uses fixed-point math: result is scaled by 1000 (1000 = 1.0x weight)
        /// 
        /// Formula: weight = clamp(log10(score + 10) / 2, 0.5, 2.0)
        /// Approximated using integer math
        fn calculate_sender_weight(sender_score: u128) -> u128 {
            // Approximate log10 using leading zeros / bit counting
            // log10(x) ≈ log2(x) / 3.32
            // We use a simpler tiered approach for efficiency:
            //   score < 10:        weight = 500  (0.5x)
            //   score 10-99:       weight = 750  (0.75x)
            //   score 100-999:     weight = 1000 (1.0x)
            //   score 1000-9999:   weight = 1500 (1.5x)
            //   score 10000+:      weight = 2000 (2.0x)
            
            if sender_score < 10 {
                MIN_SENDER_WEIGHT  // 500 = 0.5x
            } else if sender_score < 100 {
                750  // 0.75x
            } else if sender_score < 1000 {
                1000  // 1.0x
            } else if sender_score < 10000 {
                1500  // 1.5x
            } else {
                MAX_SENDER_WEIGHT  // 2000 = 2.0x
            }
        }

        /// Calculate the current period number from a block number
        fn block_to_period(block: BlockNumberFor<T>) -> u64 {
            let period_blocks: u64 = T::ClaimPeriodBlocks::get()
                .try_into()
                .unwrap_or(1);
            let block_num: u64 = block.try_into().unwrap_or(0);
            block_num / period_blocks
        }

        /// Update claim streak based on current period
        /// Returns the new streak value
        fn update_streak(rep: &mut Reputation<BlockNumberFor<T>>, current_period: u64) -> u32 {
            let periods_missed = current_period.saturating_sub(rep.last_claim_period);
            
            if periods_missed <= STREAK_GRACE_PERIODS + 1 {
                // Within grace period (0, 1, or 2 periods since last = consecutive or grace)
                // +1 because claiming in next period is periods_missed=1
                rep.claim_streak = rep.claim_streak.saturating_add(1);
            } else {
                // Streak broken - reset to 1
                rep.claim_streak = 1;
            }
            
            rep.last_claim_period = current_period;
            rep.claim_streak
        }

        /// Apply 5% decay to reputation score
        fn apply_decay(score: u128) -> u128 {
            // score * 0.95 = score * 950 / 1000
            score.saturating_mul(DECAY_FACTOR) / 1000
        }

        /// Calculate streak bonus (10 points per day, max 500)
        fn calculate_streak_bonus(streak: u32) -> u128 {
            let bonus = (streak as u128).saturating_mul(POINTS_PER_STREAK_DAY);
            bonus.min(MAX_STREAK_BONUS)
        }

        /// Recalculate the full reputation score from components
        fn recalculate_score(rep: &Reputation<BlockNumberFor<T>>) -> u128 {
            let unique_bonus = (rep.unique_recipients_count as u128)
                .saturating_mul(POINTS_PER_UNIQUE_RECIPIENT);
            
            let sent_bonus = rep.burns_sent_volume;  // 1x multiplier
            
            let received_bonus = rep.weighted_received
                .saturating_mul(WEIGHTED_RECEIVED_MULTIPLIER);
            
            let streak_bonus = Self::calculate_streak_bonus(rep.claim_streak);
            
            unique_bonus
                .saturating_add(sent_bonus)
                .saturating_add(received_bonus)
                .saturating_add(streak_bonus)
        }

        /// Get reputation score for an account (public API)
        pub fn reputation_score(who: &T::AccountId) -> u128 {
            ReputationStore::<T>::get(who).score
        }

        /// Check if sender has already burned to this recipient before
        pub fn has_burned_to(sender: &T::AccountId, recipient: &T::AccountId) -> bool {
            UniqueRecipients::<T>::get(sender, recipient)
        }
    }
}
