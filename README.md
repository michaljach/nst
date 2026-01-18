# Non Speculative Tokens (NST)

A novel Substrate-based blockchain implementing a **burn-only UBI token** designed to prevent speculation and trading. This is the first implementation of a truly non-transferable cryptocurrency with built-in Universal Basic Income distribution.

## Core Concept

Traditional cryptocurrencies allow transfers, enabling speculation and trading. NST takes a radical approach:

- **Tokens cannot be transferred** - only burned
- **Everyone receives UBI** - 100 NST per day
- **Tokens expire** - unused tokens vanish after 7 days
- **Burn = Payment** - burning tokens to an address is proof of payment

## Why It Works

```
TRADITIONAL CRYPTO:
  Alice → sends 50 tokens → Bob receives 50 tokens → Bob can sell them

NST:
  Alice → burns 50 tokens (naming Bob) → Bob sees burn event → Bob has his own UBI
  
  Result: Nothing to trade. Exchanges can't operate. Value = utility only.
```

## Features

| Feature | Description |
|---------|-------------|
| **Daily UBI** | 100 NST/day for any wallet |
| **Burn-Only** | No transfer function exists |
| **7-Day Expiry** | Unspent tokens disappear |
| **Reputation** | Track burns sent/received on-chain |
| **Open Access** | Any wallet can participate |
| **Anti-Sybil** | Expiration makes hoarding pointless |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         NST RUNTIME                             │
├─────────────────────────────────────────────────────────────────┤
│  UBI Token Pallet                                               │
│  ├── claim()           Claim daily UBI (up to 3 days backlog)   │
│  ├── burn(to, amount)  Destroy tokens, emit event for recipient │
│  └── [No transfer!]    Transfers do not exist                   │
├─────────────────────────────────────────────────────────────────┤
│  Reputation System (view-only)                                  │
│  ├── burns_sent_count      How many payments made               │
│  ├── burns_sent_volume     Total tokens burned                  │
│  ├── burns_received_count  How many payments received           │
│  ├── burns_received_volume Total tokens burned to this address  │
│  └── first_activity        Account age (block number)           │
└─────────────────────────────────────────────────────────────────┘
```

## Token Lifecycle

```
╔══════════════════════════════════════════════════════════════════╗
║                        TOKEN LIFECYCLE                           ║
╠══════════════════════════════════════════════════════════════════╣
║                                                                  ║
║   DAY 1        DAY 3              DAY 8                          ║
║     │            │                  │                            ║
║     ▼            ▼                  ▼                            ║
║   ┌────┐      ┌────┐            ┌────┐                           ║
║   │CLAIM│ ──► │BURN│ ──► OR ──► │EXPIRE│                         ║
║   └────┘      └────┘            └────┘                           ║
║     │            │                  │                            ║
║   100 NST    Pay for service    Tokens vanish                    ║
║   minted     (tokens destroyed) (if unused)                      ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
```

## Example: Buying Pizza

```
ALICE (customer)                    BOB (pizza shop)
     │                                   │
     │ claim() ─── Gets 100 NST          │ claim() ─── Gets 100 NST
     │                                   │
     │ burn(bob, 50) ──────────────────► │ Sees burn event
     │ "50 NST burned for Bob"           │ "Alice burned 50 for me"
     │                                   │
     │ ◄─────────────────────────────────│ Delivers pizza 🍕
     │                                   │
     │ Balance: 50 NST                   │ Balance: 100 NST (his own UBI)
     │                                   │ Reputation: +1 burn, +50 volume
```

**Key insight:** Bob doesn't receive Alice's tokens. He only sees proof that she burned them for him. Bob has his own UBI for his needs.

## Project Structure

```
nst/
├── Cargo.toml                    # Workspace configuration
├── pallets/
│   └── ubi-token/                # Core UBI token pallet
│       └── src/
│           ├── lib.rs            # Pallet implementation
│           ├── mock.rs           # Test configuration
│           └── tests.rs          # Unit tests
├── runtime/                      # Runtime configuration
│   └── src/lib.rs
└── node/                         # Blockchain node
    └── src/
        ├── main.rs
        ├── chain_spec.rs
        ├── cli.rs
        ├── command.rs
        ├── rpc.rs
        └── service.rs
```

## Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add wasm target
rustup target add wasm32-unknown-unknown
```

## Building

```bash
# Build the UBI token pallet
cargo build -p pallet-ubi-token

# Build the entire project
cargo build --release
```

## Testing

```bash
# Run all tests
cargo test

# Run UBI token tests specifically
cargo test -p pallet-ubi-token

# Run with output
cargo test -- --nocapture
```

## Configuration

Key parameters in `runtime/src/lib.rs`:

| Parameter | Description | Default |
|-----------|-------------|---------|
| `UbiAmount` | Tokens per claim period | 100 NST |
| `ClaimPeriodBlocks` | Blocks between claims | 14,400 (~1 day) |
| `ExpirationBlocks` | Blocks until expiry | 100,800 (~7 days) |
| `MaxBacklogPeriods` | Max claimable backlog | 3 days |

## Why Exchanges Cannot Operate

```
EXCHANGE ATTACK ATTEMPT:

1. User "deposits" by burning to exchange address
   → Exchange receives NO TOKENS (just sees event)

2. Exchange tries to sell tokens to buyer
   → Exchange has nothing to transfer!
   → No transfer function exists!

3. Exchange model = BROKEN ✓
```

## Comparison with Other UBI Projects

| Project | Transferable | Expires | Anti-Speculation |
|---------|-------------|---------|------------------|
| Circles UBI | Trust-limited | Demurrage | Web of trust |
| GoodDollar | Yes | No | Reserve model |
| Worldcoin | Yes | No | None |
| **NST** | **No** | **Yes (7 days)** | **Burn-only** |

NST is the first truly non-transferable UBI token.

## Use Cases

- **Local communities**: Circulating value without speculation
- **Platform credits**: Non-tradeable in-app currency
- **Reputation economies**: Value = social proof, not money
- **Research**: Testing novel economic models

## Roadmap

- [x] Core burn-only pallet
- [x] Reputation tracking
- [x] Expiration system
- [x] Comprehensive tests
- [ ] Node implementation
- [ ] Frontend wallet
- [ ] Mobile app
- [ ] Testnet launch

## Contributing

Contributions welcome! This is experimental software exploring new economic models.

## License

MIT

---

**NST: Because money should be used, not hoarded.**
