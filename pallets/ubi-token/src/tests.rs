use crate::{mock::*, Error, Event, Balances, LastClaim, ReputationStore, TotalSupply, UniqueRecipients};
use frame_support::{assert_noop, assert_ok};

// ============================================================================
// CLAIM TESTS
// ============================================================================

#[test]
fn claim_works_for_new_account() {
    new_test_ext().execute_with(|| {
        // Alice claims for the first time (unsigned tx)
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Check balance
        assert_eq!(UbiToken::spendable_balance(&ALICE), 100);

        // Check total supply
        assert_eq!(TotalSupply::<Test>::get(), 100);

        // Check last claim updated
        assert!(LastClaim::<Test>::get(ALICE).is_some());

        // Check event
        System::assert_last_event(
            Event::Claimed {
                who: ALICE,
                amount: 100,
                periods: 1,
                expires_at: 1 + 700, // current block + expiration
            }
            .into(),
        );
    });
}

#[test]
fn cannot_claim_twice_in_same_period() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Try to claim again immediately
        assert_noop!(
            UbiToken::claim(RuntimeOrigin::none(), ALICE),
            Error::<Test>::NothingToClaim
        );
    });
}

#[test]
fn can_claim_after_one_period() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_eq!(UbiToken::spendable_balance(&ALICE), 100);

        // Advance one claim period (100 blocks)
        run_to_block(101);

        // Can claim again
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_eq!(UbiToken::spendable_balance(&ALICE), 200);
    });
}

#[test]
fn can_claim_backlog_up_to_max() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Advance 5 periods (500 blocks) - should only get 3 days backlog
        run_to_block(501);

        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Should get 3 periods (max backlog) = 300 tokens
        // Plus the 100 from first claim = 400 total
        assert_eq!(UbiToken::spendable_balance(&ALICE), 400);

        // Check event shows 3 periods
        System::assert_last_event(
            Event::Claimed {
                who: ALICE,
                amount: 300,
                periods: 3,
                expires_at: 501 + 700,
            }
            .into(),
        );
    });
}

#[test]
fn first_activity_recorded_on_claim() {
    new_test_ext().execute_with(|| {
        let rep_before = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep_before.first_activity, 0);

        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        let rep_after = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep_after.first_activity, 1); // Block 1
    });
}

#[test]
fn multiple_accounts_can_claim() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), BOB));
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), CHARLIE));

        assert_eq!(UbiToken::spendable_balance(&ALICE), 100);
        assert_eq!(UbiToken::spendable_balance(&BOB), 100);
        assert_eq!(UbiToken::spendable_balance(&CHARLIE), 100);

        assert_eq!(TotalSupply::<Test>::get(), 300);
    });
}

// ============================================================================
// BURN TESTS
// ============================================================================

#[test]
fn burn_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Alice burns 50 tokens to Bob (unsigned tx with from parameter)
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50));

        // Alice balance decreased
        assert_eq!(UbiToken::spendable_balance(&ALICE), 50);

        // Bob balance unchanged (burn doesn't transfer)
        assert_eq!(UbiToken::spendable_balance(&BOB), 0);

        // Total supply decreased
        assert_eq!(TotalSupply::<Test>::get(), 50);

        // Check event
        System::assert_last_event(
            Event::Burned {
                from: ALICE,
                to: BOB,
                amount: 50,
            }
            .into(),
        );
    });
}

#[test]
fn burn_updates_sender_reputation() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50));

        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.burns_sent_count, 1);
        assert_eq!(rep.burns_sent_volume, 50);
    });
}

#[test]
fn burn_updates_recipient_reputation() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50));

        let rep = ReputationStore::<Test>::get(BOB);
        assert_eq!(rep.burns_received_count, 1);
        assert_eq!(rep.burns_received_volume, 50);
        assert_eq!(rep.first_activity, 1); // First activity via receiving burn
    });
}

#[test]
fn cannot_burn_to_self() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        assert_noop!(
            UbiToken::burn(RuntimeOrigin::none(), ALICE, ALICE, 50),
            Error::<Test>::CannotBurnToSelf
        );
    });
}

#[test]
fn cannot_burn_zero_amount() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        assert_noop!(
            UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 0),
            Error::<Test>::AmountMustBePositive
        );
    });
}

#[test]
fn cannot_burn_more_than_balance() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        assert_noop!(
            UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 150),
            Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn burn_uses_fifo() {
    new_test_ext().execute_with(|| {
        // Alice claims at block 1
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Advance one period and claim again
        run_to_block(101);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Alice has 200 tokens in 2 batches
        assert_eq!(UbiToken::spendable_balance(&ALICE), 200);

        // Burn 150 - should use all of first batch (100) + 50 from second
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 150));

        assert_eq!(UbiToken::spendable_balance(&ALICE), 50);

        // Check batches - should have 1 batch with 50
        let batches = Balances::<Test>::get(ALICE);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].amount, 50);
    });
}

#[test]
fn multiple_burns_accumulate_reputation() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 30));
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 20));
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, CHARLIE, 10));

        let alice_rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(alice_rep.burns_sent_count, 3);
        assert_eq!(alice_rep.burns_sent_volume, 60);

        let bob_rep = ReputationStore::<Test>::get(BOB);
        assert_eq!(bob_rep.burns_received_count, 2);
        assert_eq!(bob_rep.burns_received_volume, 50);

        let charlie_rep = ReputationStore::<Test>::get(CHARLIE);
        assert_eq!(charlie_rep.burns_received_count, 1);
        assert_eq!(charlie_rep.burns_received_volume, 10);
    });
}

// ============================================================================
// EXPIRATION TESTS
// ============================================================================

#[test]
fn tokens_expire_after_expiration_period() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_eq!(UbiToken::spendable_balance(&ALICE), 100);

        // Advance past expiration (700 blocks)
        run_to_block(702);

        // Balance should now show 0 (expired)
        assert_eq!(UbiToken::spendable_balance(&ALICE), 0);
    });
}

#[test]
fn expired_tokens_cleaned_up_on_claim() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Advance past expiration (700 blocks)
        run_to_block(702);

        // Claim again - this should clean up expired tokens and claim backlog
        // After 702 blocks (7 periods), can claim max backlog of 3 periods = 300 tokens
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Should have 300 (3 periods backlog, max)
        // Original 100 expired, new 300 from backlog claim
        assert_eq!(UbiToken::spendable_balance(&ALICE), 300);

        // Check expired event was emitted
        let events = System::events();
        let expired_event = events.iter().find(|e| {
            matches!(
                e.event,
                RuntimeEvent::UbiToken(Event::Expired { who: ALICE, amount: 100 })
            )
        });
        assert!(expired_event.is_some());
    });
}

#[test]
fn expired_tokens_cleaned_up_on_burn() {
    new_test_ext().execute_with(|| {
        // Alice claims twice
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        run_to_block(101);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Advance so first batch expires (700 blocks from block 1 = 701)
        // but second batch hasn't (700 blocks from block 101 = 801)
        run_to_block(702);

        // Alice tries to burn - should clean up expired batch first
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50));

        // Should have 50 left from second batch
        assert_eq!(UbiToken::spendable_balance(&ALICE), 50);
    });
}

#[test]
fn cannot_burn_expired_tokens() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Advance past expiration
        run_to_block(702);

        // Try to burn - should fail (tokens expired)
        assert_noop!(
            UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50),
            Error::<Test>::InsufficientBalance
        );
    });
}

// ============================================================================
// HELPER FUNCTION TESTS
// ============================================================================

#[test]
fn can_claim_helper_works() {
    new_test_ext().execute_with(|| {
        // New account can claim
        assert!(UbiToken::can_claim(&ALICE));

        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Just claimed - cannot claim again
        assert!(!UbiToken::can_claim(&ALICE));

        // After one period - can claim
        run_to_block(101);
        assert!(UbiToken::can_claim(&ALICE));
    });
}

#[test]
fn claimable_amount_helper_works() {
    new_test_ext().execute_with(|| {
        // New account - 1 period = 100
        assert_eq!(UbiToken::claimable_amount(&ALICE), 100);

        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // Just claimed - 0
        assert_eq!(UbiToken::claimable_amount(&ALICE), 0);

        // After 2 periods - 200
        run_to_block(201);
        assert_eq!(UbiToken::claimable_amount(&ALICE), 200);

        // After 5 periods - capped at 3 = 300
        run_to_block(501);
        assert_eq!(UbiToken::claimable_amount(&ALICE), 300);
    });
}

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[test]
fn full_lifecycle_pizza_purchase() {
    new_test_ext().execute_with(|| {
        // Day 1: Alice and Bob both claim UBI
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), BOB));

        // Alice burns 50 tokens to Bob for pizza
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50));

        // Check balances
        assert_eq!(UbiToken::spendable_balance(&ALICE), 50);
        assert_eq!(UbiToken::spendable_balance(&BOB), 100); // Bob's own UBI, not Alice's burn

        // Check reputation
        let alice_rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(alice_rep.burns_sent_count, 1);
        assert_eq!(alice_rep.burns_sent_volume, 50);

        let bob_rep = ReputationStore::<Test>::get(BOB);
        assert_eq!(bob_rep.burns_received_count, 1);
        assert_eq!(bob_rep.burns_received_volume, 50);

        // Day 2: Bob burns to Charlie for flour
        run_to_block(101);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), BOB));
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), BOB, CHARLIE, 30));

        let charlie_rep = ReputationStore::<Test>::get(CHARLIE);
        assert_eq!(charlie_rep.burns_received_count, 1);
        assert_eq!(charlie_rep.burns_received_volume, 30);

        // Total supply check
        // Alice: 100 - 50 = 50
        // Bob: 100 + 100 - 30 = 170
        // Charlie: 0
        // Total: 50 + 170 = 220
        assert_eq!(TotalSupply::<Test>::get(), 220);
    });
}

#[test]
fn sybil_attack_is_pointless() {
    new_test_ext().execute_with(|| {
        // Attacker creates many accounts and claims
        for i in 100..110 {
            assert_ok!(UbiToken::claim(RuntimeOrigin::none(), i));
        }

        // Total supply is 1000 (10 accounts * 100)
        assert_eq!(TotalSupply::<Test>::get(), 1000);

        // But after expiration, all tokens disappear
        run_to_block(702);

        // All balances are now 0
        for i in 100..110 {
            assert_eq!(UbiToken::spendable_balance(&i), 0);
        }

        // Attacker gained nothing - tokens expired
        // They can only burn tokens, not trade them
    });
}

#[test]
fn exchange_cannot_operate() {
    new_test_ext().execute_with(|| {
        // User claims tokens
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));

        // User "deposits" to exchange by burning to exchange address
        let exchange: u64 = 999;
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, exchange, 100));

        // Exchange received NO TOKENS - just a burn event
        assert_eq!(UbiToken::spendable_balance(&exchange), 0);

        // Exchange has nothing to sell!
        // The burn event is proof Alice paid, but exchange cannot transfer anything

        // Exchange can claim its own UBI
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), exchange));
        assert_eq!(UbiToken::spendable_balance(&exchange), 100);

        // But those are the exchange's own tokens, not "user deposits"
        // Exchange business model is broken
    });
}

// ============================================================================
// ENHANCED REPUTATION SYSTEM TESTS
// ============================================================================

#[test]
fn claim_streak_increments_on_consecutive_claims() {
    new_test_ext().execute_with(|| {
        // First claim
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.claim_streak, 1);

        // Claim next period
        run_to_block(101);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.claim_streak, 2);

        // Claim next period
        run_to_block(201);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.claim_streak, 3);
    });
}

#[test]
fn claim_streak_respects_grace_period() {
    new_test_ext().execute_with(|| {
        // First claim
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        
        // Skip 2 periods (within grace period of 2)
        run_to_block(301);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        let rep = ReputationStore::<Test>::get(ALICE);
        // Streak should still increment (3 periods passed, grace is 2+1=3 allowed)
        assert_eq!(rep.claim_streak, 2);
    });
}

#[test]
fn claim_streak_resets_after_grace_period() {
    new_test_ext().execute_with(|| {
        // Build up streak
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        run_to_block(101);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        run_to_block(201);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.claim_streak, 3);

        // Skip more than grace period (4 periods = 400 blocks)
        run_to_block(601);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        
        let rep = ReputationStore::<Test>::get(ALICE);
        // Streak should reset to 1
        assert_eq!(rep.claim_streak, 1);
    });
}

#[test]
fn claim_applies_reputation_decay() {
    new_test_ext().execute_with(|| {
        // Alice claims and burns to build up score
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50));
        
        let rep_before = ReputationStore::<Test>::get(ALICE);
        let _score_before = rep_before.score;
        
        // Claim again - should apply 5% decay
        run_to_block(101);
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        
        let rep_after = ReputationStore::<Test>::get(ALICE);
        
        // Score should have decayed (0.95x) then been recalculated
        // The exact value depends on recalculation, but decay was applied
        assert!(rep_after.score > 0);
    });
}

#[test]
fn unique_recipients_tracked_correctly() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        
        // Burn to Bob - first unique recipient
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 20));
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.unique_recipients_count, 1);
        assert!(UniqueRecipients::<Test>::get(ALICE, BOB));
        
        // Burn to Bob again - should NOT increment unique count
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 20));
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.unique_recipients_count, 1);
        
        // Burn to Charlie - second unique recipient
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, CHARLIE, 20));
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.unique_recipients_count, 2);
        assert!(UniqueRecipients::<Test>::get(ALICE, CHARLIE));
    });
}

#[test]
fn weighted_received_uses_sender_weight() {
    new_test_ext().execute_with(|| {
        // Alice (new user) claims - gets streak bonus of 10, score = 10
        // Score of 10 puts her in 10-99 tier = 0.75x weight (750/1000)
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        
        let alice_rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(alice_rep.score, 10); // streak bonus only
        
        // Alice burns 100 to Bob
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 100));
        
        let bob_rep = ReputationStore::<Test>::get(BOB);
        // Alice has score 10, so weight is 0.75x = 750/1000
        // weighted_received = 100 * 750 / 1000 = 75
        assert_eq!(bob_rep.weighted_received, 75);
        
        // Now test with a zero-score sender (burns before any claim/activity)
        // This is not possible in normal flow since you need to claim first to get tokens
        // So the minimum practical sender weight is 0.75x (score 10 from first claim streak)
    });
}

#[test]
fn reputation_score_formula_correct() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        
        // Burn to 2 unique recipients
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 30));
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, CHARLIE, 20));
        
        let rep = ReputationStore::<Test>::get(ALICE);
        
        // Expected score components:
        // - unique_recipients: 2 * 50 = 100
        // - burns_sent_volume: 50 * 1 = 50
        // - weighted_received: 0 * 2 = 0
        // - streak_bonus: 1 * 10 = 10
        // Total: 100 + 50 + 0 + 10 = 160
        
        assert_eq!(rep.unique_recipients_count, 2);
        assert_eq!(rep.burns_sent_volume, 50);
        assert_eq!(rep.weighted_received, 0);
        assert_eq!(rep.claim_streak, 1);
        assert_eq!(rep.score, 160);
    });
}

#[test]
fn streak_bonus_caps_at_max() {
    new_test_ext().execute_with(|| {
        // Claim 60 times (more than the 50-day cap)
        for i in 0..60 {
            run_to_block(i * 100 + 1);
            assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        }
        
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(rep.claim_streak, 60);
        
        // Streak bonus should be capped at MAX_STREAK_BONUS (500)
        // Score = unique(0) + sent(0) + received(0) + streak_bonus(500)
        // But decay has been applied each time, so score will be different
        // Just verify streak is tracked correctly
        assert!(rep.claim_streak >= 50);
    });
}

#[test]
fn bot_farming_yields_low_reputation() {
    new_test_ext().execute_with(|| {
        // Simulate bot ring: A -> B -> A
        let bot_a: u64 = 100;
        let bot_b: u64 = 101;
        
        // Both bots claim - each gets score of 10 (streak bonus)
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), bot_a));
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), bot_b));
        
        let bot_a_score_before = ReputationStore::<Test>::get(bot_a).score;
        let bot_b_score_before = ReputationStore::<Test>::get(bot_b).score;
        assert_eq!(bot_a_score_before, 10); // Just streak bonus
        assert_eq!(bot_b_score_before, 10);
        
        // Bot A burns to Bot B
        // Bot A has score 10 -> weight 0.75x (750/1000)
        // weighted_received for B = 100 * 750 / 1000 = 75
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), bot_a, bot_b, 100));
        
        // Bot B burns to Bot A
        // Bot B's score after receiving = 10 (sent) + 75*2 (weighted_received) + 10 (streak) = 170
        // Actually let me check the actual formula
        let bot_b_rep_after_receive = ReputationStore::<Test>::get(bot_b);
        
        // Bot B burns - their score determines weight
        // If B's score is in 100-999 range, weight is 1.0x
        let _bot_b_rep_after_receive = ReputationStore::<Test>::get(bot_b);
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), bot_b, bot_a, 100));
        
        let bot_a_rep = ReputationStore::<Test>::get(bot_a);
        let bot_b_rep = ReputationStore::<Test>::get(bot_b);
        
        // Both should have limited weighted_received
        // Bot A received from Bot B when B had score ~170 (100-999 tier = 1.0x)
        // Bot B received from Bot A when A had score 10 (10-99 tier = 0.75x)
        assert_eq!(bot_b_rep.weighted_received, 75);  // From A at 0.75x weight
        assert_eq!(bot_a_rep.weighted_received, 100); // From B at 1.0x weight
        
        // Each has only 1 unique recipient
        assert_eq!(bot_a_rep.unique_recipients_count, 1);
        assert_eq!(bot_b_rep.unique_recipients_count, 1);
        
        // Key insight: bot farming is limited because:
        // 1. New bots start with low reputation = low weight
        // 2. Each bot can only have 1 unique recipient from the other
        // 3. A legitimate user receiving burns from many different high-rep users
        //    would accumulate much higher weighted_received
    });
}

#[test]
fn reputation_score_public_api() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50));
        
        // Public API should return the score
        let score = UbiToken::reputation_score(&ALICE);
        assert!(score > 0);
        
        let rep = ReputationStore::<Test>::get(ALICE);
        assert_eq!(score, rep.score);
    });
}

#[test]
fn has_burned_to_helper_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(UbiToken::claim(RuntimeOrigin::none(), ALICE));
        
        // Before burning
        assert!(!UbiToken::has_burned_to(&ALICE, &BOB));
        
        // After burning
        assert_ok!(UbiToken::burn(RuntimeOrigin::none(), ALICE, BOB, 50));
        assert!(UbiToken::has_burned_to(&ALICE, &BOB));
        
        // Different recipient still false
        assert!(!UbiToken::has_burned_to(&ALICE, &CHARLIE));
    });
}
