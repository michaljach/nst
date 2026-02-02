# Non Speculative Tokens (NST)

A novel Substrate-based blockchain implementing a **burn-only UBI token** designed to prevent speculation and trading. This is the first implementation of a truly non-transferable cryptocurrency with built-in Universal Basic Income distribution.

## Screenshots

<table>
  <tr>
    <td><img src="screenshots/Screenshot 2026-02-02 at 19.37.21.png" width="100%" alt="NST Wallet Interface"/></td>
    <td><img src="screenshots/Screenshot 2026-02-02 at 19.38.54.png" width="100%" alt="NST Transaction View"/></td>
  </tr>
</table>

## Core Concept

Traditional cryptocurrencies allow transfers, enabling speculation and trading. NST takes a radical approach:

- **Tokens cannot be transferred** - only burned
- **Everyone receives UBI** - 100 NST per day
- **Tokens expire** - unused tokens vanish after 7 days
- **Burn = Payment** - burning tokens to an address is proof of payment
- **All transactions are FREE** - no gas fees required

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
| **Reputation** | Volume-based on-chain reputation |
| **Free Transactions** | No gas fees - truly accessible |
| **Open Access** | Any wallet can participate |
| **Anti-Sybil** | Expiration makes hoarding pointless |

## Reputation System

Reputation is **volume-based** - it rewards actual economic value creation, not transaction frequency.

### Formula

```
Reputation Score = Tokens Burned + (Tokens Received × 2)
```

**Why receiving is valued 2x more:**
- Reputation is earned by **others choosing to pay you**
- Being useful to the community matters more than spending
- A pizza seller who receives 500 NST from customers has higher reputation than someone who just spends

### Reputation Labels

| Score | Label |
|-------|-------|
| 0 | Newcomer |
| 100+ | Getting Started |
| 500+ | Active Member |
| 2,000+ | Trusted Contributor |
| 5,000+ | Community Pillar |
| 10,000+ | Local Legend |
| 25,000+ | Community Elder |

### Example

```
Pizza Seller (provides value):
  - Receives 500 NST from customers
  - Burns 100 NST on supplies
  - Score: 100 + (500 × 2) = 1,100 (Trusted Contributor)

Customer:
  - Burns 500 NST on pizzas
  - Receives 100 NST for odd jobs
  - Score: 500 + (100 × 2) = 700 (Active Member)
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         NST RUNTIME                             │
├─────────────────────────────────────────────────────────────────┤
│  UBI Token Pallet (FREE - unsigned transactions)                │
│  ├── claim(account)      Claim daily UBI (up to 3 days backlog) │
│  ├── burn(from, to, amt) Destroy tokens, emit event for recipient│
│  └── [No transfer!]      Transfers do not exist                 │
├─────────────────────────────────────────────────────────────────┤
│  Reputation System (view-only, volume-based)                    │
│  ├── burns_sent_volume       Total tokens burned (given)        │
│  ├── burns_received_volume   Total tokens burned to this address│
│  ├── burns_sent_count        Number of payments made            │
│  ├── burns_received_count    Number of payments received        │
│  └── first_activity          Account age (block number)         │
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
     │ ◄─────────────────────────────────│ Delivers pizza
     │                                   │
     │ Balance: 50 NST                   │ Balance: 100 NST (his own UBI)
     │ Reputation: +50 burned            │ Reputation: +100 received (+200 score)
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
├── node/                         # Blockchain node
│   └── src/
│       ├── main.rs
│       ├── chain_spec.rs
│       ├── cli.rs
│       ├── command.rs
│       ├── rpc.rs
│       └── service.rs
└── frontend/                     # React wallet UI
    └── src/
        ├── App.tsx
        └── App.css
```

## Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add wasm target
rustup target add wasm32-unknown-unknown

# For frontend
npm install (in frontend directory)
```

## Building

```bash
# Build the UBI token pallet
cargo build -p pallet-ubi-token

# Build the entire project (release)
cargo build -p nst-node --release

# Build frontend
cd frontend && npm install
```

## Running

```bash
# Terminal 1: Start the node
./target/release/nst-node --dev --tmp

# Terminal 2: Start the frontend
cd frontend && npm run dev

# Open http://localhost:5173 and connect your wallet
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
| `UbiAmount` | Tokens per claim period | 100 NST (9 decimals) |
| `ClaimPeriodBlocks` | Blocks between claims | 10 (dev) / 14,400 (~1 day) |
| `ExpirationBlocks` | Blocks until expiry | 70 (dev) / 100,800 (~7 days) |
| `MaxBacklogPeriods` | Max claimable backlog | 3 periods |

## Why Exchanges Cannot Operate

```
EXCHANGE ATTACK ATTEMPT:

1. User "deposits" by burning to exchange address
   → Exchange receives NO TOKENS (just sees event)

2. Exchange tries to sell tokens to buyer
   → Exchange has nothing to transfer!
   → No transfer function exists!

3. Exchange model = BROKEN
```

## Comparison with Other UBI Projects

| Project | Transferable | Expires | Anti-Speculation | Free Transactions |
|---------|-------------|---------|------------------|-------------------|
| Circles UBI | Trust-limited | Demurrage | Web of trust | No |
| GoodDollar | Yes | No | Reserve model | No |
| Worldcoin | Yes | No | None | No |
| **NST** | **No** | **Yes (7 days)** | **Burn-only** | **Yes** |

NST is the first truly non-transferable, fee-free UBI token.

## Use Cases

- **Local communities**: Circulating value without speculation
- **Platform credits**: Non-tradeable in-app currency
- **Reputation economies**: Value = social proof, not money
- **Research**: Testing novel economic models

## Roadmap

- [x] Core burn-only pallet
- [x] Reputation tracking (volume-based)
- [x] Expiration system
- [x] Comprehensive tests
- [x] Node implementation
- [x] Frontend wallet
- [x] Free transactions (unsigned)
- [ ] Mobile app
- [ ] Testnet launch

## Contributing

Contributions welcome! This is experimental software exploring new economic models.

## License

MIT

---

**NST: Because money should be used, not hoarded.**
