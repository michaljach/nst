import { useState, useEffect, useCallback, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { web3Accounts, web3Enable } from '@polkadot/extension-dapp';
import type { InjectedAccountWithMeta } from '@polkadot/extension-inject/types';
import './App.css';

interface TokenBatch {
  amount: string;
  expiresAtBlock: number;
}

interface Reputation {
  totalBurned: string;
  totalReceived: string;
  burnCount: number;
  receiveCount: number;
  firstActivity: number;
}

interface BlockInfo {
  number: number;
  hash: string;
  parentHash: string;
  extrinsicsCount: number;
  timestamp: number;
}

interface ChainEvent {
  blockNumber: number;
  section: string;
  method: string;
  data: string;
  timestamp: number;
}

interface ChainStats {
  totalSupply: string;
  totalAccounts: number;
  avgBlockTime: number;
  finalizedBlock: number;
}

function App() {
  const navigate = useNavigate();
  const [api, setApi] = useState<ApiPromise | null>(null);
  const [accounts, setAccounts] = useState<InjectedAccountWithMeta[]>([]);
  const [selectedAccount, setSelectedAccount] = useState<string>('');
  const [connected, setConnected] = useState(false);
  const [blockNumber, setBlockNumber] = useState<number>(0);
  
  // UBI State
  const [balance, setBalance] = useState<TokenBatch[]>([]);
  const [totalBalance, setTotalBalance] = useState<string>('0');
  const [claimableAmount, setClaimableAmount] = useState<string>('0');
  const [canClaim, setCanClaim] = useState(false);
  const [reputation, setReputation] = useState<Reputation | null>(null);
  
  // Burn form
  const [burnRecipient, setBurnRecipient] = useState('');
  const [burnAmount, setBurnAmount] = useState('');
  
  // Status
  const [status, setStatus] = useState('');
  const [loading, setLoading] = useState(false);

  // Blockchain explorer state
  const [recentBlocks, setRecentBlocks] = useState<BlockInfo[]>([]);
  const [recentEvents, setRecentEvents] = useState<ChainEvent[]>([]);
  const [chainStats, setChainStats] = useState<ChainStats>({
    totalSupply: '0',
    totalAccounts: 0,
    avgBlockTime: 6,
    finalizedBlock: 0,
  });
  const [showExplorer, setShowExplorer] = useState(true);
  const lastBlockTimeRef = useRef<number>(Date.now());
  const blockTimesRef = useRef<number[]>([]);

  // Fetch historical blocks and events from chain
  const fetchHistoricalData = useCallback(async (api: ApiPromise, currentBlock: number) => {
    const blocks: BlockInfo[] = [];
    const events: ChainEvent[] = [];
    
    // Fetch last 50 blocks to find events (they may be sparse)
    const startBlock = Math.max(1, currentBlock - 49);
    
    console.log(`Fetching blocks ${startBlock} to ${currentBlock}`);
    
    for (let blockNum = currentBlock; blockNum >= startBlock; blockNum--) {
      try {
        const blockHash = await api.rpc.chain.getBlockHash(blockNum);
        const signedBlock = await api.rpc.chain.getBlock(blockHash);
        const header = signedBlock.block.header;
        
        // Only keep last 8 blocks for display
        if (blocks.length < 8) {
          blocks.push({
            number: blockNum,
            hash: blockHash.toHex(),
            parentHash: header.parentHash.toHex(),
            extrinsicsCount: signedBlock.block.extrinsics.length,
            timestamp: Date.now() - ((currentBlock - blockNum) * 6000),
          });
        }
        
        // Get events for this block
        const apiAt = await api.at(blockHash);
        const blockEvents = await apiAt.query.system.events();
        
        blockEvents.forEach((record: any) => {
          const { event } = record;
          console.log(`Block ${blockNum} event: ${event.section}.${event.method}`);
          if (event.section === 'ubiToken') {
            console.log('Found UBI event:', event.method, event.data.toString());
            events.push({
              blockNumber: blockNum,
              section: event.section,
              method: event.method,
              data: event.data.toString(),
              timestamp: Date.now() - ((currentBlock - blockNum) * 6000),
            });
          }
        });
      } catch (e) {
        console.error(`Error fetching block ${blockNum}:`, e);
      }
    }
    
    console.log(`Found ${events.length} UBI events`);
    setRecentBlocks(blocks.slice(0, 8));
    setRecentEvents(events.slice(0, 10));
  }, []);

  // Connect to node
  useEffect(() => {
    const connect = async () => {
      try {
        setStatus('Connecting to NST node...');
        const provider = new WsProvider('ws://127.0.0.1:9944');
        const api = await ApiPromise.create({ provider });
        setApi(api);
        setConnected(true);
        setStatus('Connected to NST node');
        
        // Get current block and fetch historical data
        const currentHeader = await api.rpc.chain.getHeader();
        const currentBlock = currentHeader.number.toNumber();
        setBlockNumber(currentBlock);
        
        // Fetch historical blocks and events
        await fetchHistoricalData(api, currentBlock);
        
        // Subscribe to new blocks
        await api.rpc.chain.subscribeNewHeads(async (header) => {
          const blockNum = header.number.toNumber();
          setBlockNumber(blockNum);
          
          // Calculate block time
          const now = Date.now();
          const timeDiff = (now - lastBlockTimeRef.current) / 1000;
          lastBlockTimeRef.current = now;
          
          // Keep last 10 block times for average
          blockTimesRef.current.push(timeDiff);
          if (blockTimesRef.current.length > 10) {
            blockTimesRef.current.shift();
          }
          const avgBlockTime = blockTimesRef.current.reduce((a, b) => a + b, 0) / blockTimesRef.current.length;
          
          // Get block details
          const blockHash = header.hash.toHex();
          const signedBlock = await api.rpc.chain.getBlock(blockHash);
          const extrinsicsCount = signedBlock.block.extrinsics.length;
          
          const newBlock: BlockInfo = {
            number: blockNum,
            hash: blockHash,
            parentHash: header.parentHash.toHex(),
            extrinsicsCount,
            timestamp: now,
          };
          
          setRecentBlocks(prev => {
            // Prevent duplicates
            if (prev.some(b => b.number === newBlock.number)) {
              return prev;
            }
            return [newBlock, ...prev].slice(0, 8);
          });
          
          // Get events for this block
          const apiAt = await api.at(blockHash);
          const events = await apiAt.query.system.events();
          
          const blockEvents: ChainEvent[] = [];
          events.forEach((record: any) => {
            const { event } = record;
            // Filter for interesting events (UBI token events)
            if (event.section === 'ubiToken') {
              blockEvents.push({
                blockNumber: blockNum,
                section: event.section,
                method: event.method,
                data: event.data.toString(),
                timestamp: now,
              });
            }
          });
          
          if (blockEvents.length > 0) {
            setRecentEvents(prev => [...blockEvents, ...prev].slice(0, 10));
          }
          
          // Update chain stats
          try {
            const totalSupply = await api.query.ubiToken.totalSupply();
            const finalizedHead = await api.rpc.chain.getFinalizedHead();
            const finalizedHeader = await api.rpc.chain.getHeader(finalizedHead);
            
            setChainStats(prev => ({
              ...prev,
              totalSupply: totalSupply.toString(),
              avgBlockTime: Math.round(avgBlockTime * 10) / 10,
              finalizedBlock: finalizedHeader.number.toNumber(),
            }));
          } catch (e) {
            console.error('Error fetching chain stats:', e);
          }
        });
      } catch (err) {
        setStatus(`Failed to connect: ${err}`);
      }
    };
    connect();
  }, [fetchHistoricalData]);

  // Connect wallet
  const connectWallet = async () => {
    try {
      setStatus('Connecting wallet...');
      const extensions = await web3Enable('NST Wallet');
      if (extensions.length === 0) {
        setStatus('No wallet extension found. Please install Polkadot.js extension.');
        return;
      }
      
      const allAccounts = await web3Accounts();
      setAccounts(allAccounts);
      if (allAccounts.length > 0) {
        setSelectedAccount(allAccounts[0].address);
        setStatus(`Found ${allAccounts.length} account(s)`);
      } else {
        setStatus('No accounts found in wallet');
      }
    } catch (err) {
      setStatus(`Wallet error: ${err}`);
    }
  };

  // Fetch account data
  const fetchAccountData = useCallback(async () => {
    if (!api || !selectedAccount || blockNumber === 0) return;
    
    try {
      console.log('Fetching data for account:', selectedAccount, 'at block:', blockNumber);
      
      // Get token batches
      const balances = await api.query.ubiToken.balances(selectedAccount);
      const batchesRaw = balances.toJSON() as any[];
      console.log('Raw balances:', JSON.stringify(batchesRaw, null, 2));
      const batches: TokenBatch[] = batchesRaw?.map((b: any) => {
        console.log('Batch item:', b);
        return {
          amount: (b.amount ?? b[0] ?? '0').toString(),
          expiresAtBlock: b.expiresAt ?? b.expires_at ?? b[1] ?? 0,
        };
      }) || [];
      console.log('Parsed batches:', batches);
      setBalance(batches);
      
      // Calculate total valid balance
      const total = batches
        .filter(b => b.expiresAtBlock > blockNumber)
        .reduce((sum, b) => sum + BigInt(b.amount), BigInt(0));
      setTotalBalance(total.toString());
      
      // Check if can claim by reading LastClaim storage
      // User can claim if they haven't claimed in the current period
      const lastClaimBlock = await api.query.ubiToken.lastClaim(selectedAccount);
      const lastClaim = lastClaimBlock.toJSON() as number | null;
      console.log('Last claim block:', lastClaim);
      
      // Get claim period from constants (default 10 blocks in dev)
      const claimPeriod = 10; // TODO: read from runtime constants
      
      if (lastClaim === null) {
        // Never claimed before - can claim
        console.log('Never claimed - can claim now');
        setCanClaim(true);
        setClaimableAmount((100 * 10**9).toString()); // 100 tokens with 9 decimals
      } else {
        // Check if enough blocks have passed
        const blocksSinceClaim = blockNumber - lastClaim;
        const periodsClaimable = Math.floor(blocksSinceClaim / claimPeriod);
        const canClaimNow = periodsClaimable > 0;
        console.log('Blocks since claim:', blocksSinceClaim, 'Periods claimable:', periodsClaimable);
        setCanClaim(canClaimNow);
        // Cap at 3 periods (max backlog)
        const periods = Math.min(periodsClaimable, 3);
        setClaimableAmount((periods * 100 * 10**9).toString());
      }
      
      // Get reputation
      const rep = await api.query.ubiToken.reputationStore(selectedAccount);
      const repJson = rep.toJSON() as any;
      console.log('Reputation raw:', repJson);
      if (repJson) {
        setReputation({
          totalBurned: (repJson.burnsSentVolume ?? repJson.burns_sent_volume ?? '0').toString(),
          totalReceived: (repJson.burnsReceivedVolume ?? repJson.burns_received_volume ?? '0').toString(),
          burnCount: repJson.burnsSentCount ?? repJson.burns_sent_count ?? 0,
          receiveCount: repJson.burnsReceivedCount ?? repJson.burns_received_count ?? 0,
          firstActivity: repJson.firstActivity ?? repJson.first_activity ?? 0,
        });
      }
    } catch (err) {
      console.error('Error fetching data:', err);
    }
  }, [api, selectedAccount, blockNumber]);

  useEffect(() => {
    fetchAccountData();
  }, [fetchAccountData]);

  // Claim UBI (FREE - unsigned transaction, no wallet signature needed)
  const claimUBI = async () => {
    if (!api || !selectedAccount) return;
    
    setLoading(true);
    setStatus('Claiming UBI...');
    
    try {
      // Create the call
      const tx = api.tx.ubiToken.claim(selectedAccount);
      console.log('TX hex:', tx.toHex());
      console.log('TX method:', tx.method.toHex());
      
      // Submit unsigned extrinsic via RPC
      const hash = await api.rpc.author.submitExtrinsic(tx.toHex());
      console.log('Submitted hash:', hash.toHex());
      setStatus(`Claim submitted with hash: ${hash.toHex()}`);
      
      // Wait a bit and refresh
      setTimeout(() => {
        fetchAccountData();
        setLoading(false);
        setStatus('Claim processed!');
      }, 6000);
    } catch (err: any) {
      console.error('Claim error:', err);
      console.error('Error message:', err?.message);
      console.error('Error data:', err?.data);
      setStatus(`Claim failed: ${err?.message || err}`);
      setLoading(false);
    }
  };

  // Burn tokens (FREE - unsigned transaction, no wallet signature needed)
  const burnTokens = async () => {
    if (!api || !selectedAccount || !burnRecipient || !burnAmount) return;
    
    setLoading(true);
    setStatus('Burning tokens...');
    
    try {
      const amount = BigInt(burnAmount) * BigInt(10 ** 9); // 9 decimals
      
      // Create unsigned extrinsic and submit directly
      const tx = api.tx.ubiToken.burn(selectedAccount, burnRecipient, amount.toString());
      const extrinsic = api.createType('Extrinsic', tx);
      
      // Submit unsigned extrinsic via RPC
      const hash = await api.rpc.author.submitExtrinsic(extrinsic);
      setStatus(`Burn submitted with hash: ${hash.toHex()}`);
      
      // Wait a bit and refresh
      setTimeout(() => {
        fetchAccountData();
        setBurnRecipient('');
        setBurnAmount('');
        setLoading(false);
        setStatus('Burn processed!');
      }, 6000);
    } catch (err) {
      console.error('Burn error:', err);
      setStatus(`Burn failed: ${err}`);
      setLoading(false);
    }
  };

  const formatTokens = (amount: string) => {
    const num = BigInt(amount);
    const whole = num / BigInt(10 ** 9); // 9 decimals
    return whole.toString();
  };

  // Calculate reputation score based on volume
  // Receiving valued 2x - reputation is earned by being useful to others
  const calculateReputationScore = (rep: Reputation): number => {
    const burned = Number(BigInt(rep.totalBurned) / BigInt(10 ** 9));
    const received = Number(BigInt(rep.totalReceived) / BigInt(10 ** 9));
    return burned + (received * 2);
  };

  // Get reputation label based on score
  const getReputationLabel = (score: number): string => {
    if (score === 0) return 'Newcomer';
    if (score < 100) return 'Getting Started';
    if (score < 500) return 'Active Member';
    if (score < 2000) return 'Trusted Contributor';
    if (score < 5000) return 'Community Pillar';
    if (score < 10000) return 'Local Legend';
    if (score < 25000) return 'Community Elder';
    return 'Legend';
  };

  // Format hash for display
  const formatHash = (hash: string): string => {
    return `${hash.slice(0, 10)}...${hash.slice(-8)}`;
  };

  // Format time ago
  const formatTimeAgo = (timestamp: number): string => {
    const seconds = Math.floor((Date.now() - timestamp) / 1000);
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    return `${hours}h ago`;
  };

  // Format event data for display
  const formatEventData = (method: string, data: string): string => {
    try {
      const parts = data.replace(/[\[\]]/g, '').split(',').map(s => s.trim());
      switch (method) {
        case 'Claimed':
          return `${formatHash(parts[0])} claimed ${formatTokens(parts[1])} NST`;
        case 'Burned':
          return `${formatHash(parts[0])} burned ${formatTokens(parts[2])} NST to ${formatHash(parts[1])}`;
        case 'TokensExpired':
          return `${formatTokens(parts[1])} NST expired for ${formatHash(parts[0])}`;
        default:
          return data.length > 50 ? data.slice(0, 50) + '...' : data;
      }
    } catch {
      return data.length > 50 ? data.slice(0, 50) + '...' : data;
    }
  };

  return (
    <div className="container">
      <header>
        <h1>NST Wallet</h1>
        <p className="subtitle">Non Speculative Tokens - Burn-Only UBI</p>
      </header>

      <div className="status-bar">
        <span className={`connection ${connected ? 'connected' : ''}`}>
          {connected ? 'Connected' : 'Disconnected'}
        </span>
        <span className="block">Block: #{blockNumber}</span>
      </div>

      {!selectedAccount ? (
        <button className="connect-btn" onClick={connectWallet}>
          Connect Wallet
        </button>
      ) : (
        <>
          <div className="account-selector">
            <label>Account:</label>
            <select 
              value={selectedAccount} 
              onChange={(e) => setSelectedAccount(e.target.value)}
            >
              {accounts.map((acc) => (
                <option key={acc.address} value={acc.address}>
                  {acc.meta.name || 'Account'} - {acc.address.slice(0, 8)}...{acc.address.slice(-6)}
                </option>
              ))}
            </select>
          </div>

          <div className="card balance-card">
            <h2>Your Balance</h2>
            <div className="balance-amount">{formatTokens(totalBalance)} NST</div>
            <div className="balance-detail">
              {balance.filter(b => b.expiresAtBlock > blockNumber).length} active batch(es)
            </div>
            
            {balance.length > 0 && (
              <div className="batches">
                <h4>Token Batches:</h4>
                {balance.map((batch, i) => (
                  <div 
                    key={i} 
                    className={`batch ${batch.expiresAtBlock <= blockNumber ? 'expired' : ''}`}
                  >
                    <span>{formatTokens(batch.amount)} NST</span>
                    <span className="expires">
                      {batch.expiresAtBlock <= blockNumber 
                        ? 'EXPIRED' 
                        : `Expires block #${batch.expiresAtBlock}`}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="card claim-card">
            <h2>Claim UBI</h2>
            <p>Receive 100 NST/day (expires in 7 days) - FREE, no gas fees!</p>
            <div className="claim-info">
              <span>Claimable: {formatTokens(claimableAmount)} NST</span>
            </div>
            <button 
              onClick={claimUBI} 
              disabled={!canClaim || loading}
              className="action-btn"
            >
              {loading ? 'Processing...' : canClaim ? 'Claim Now' : 'Already Claimed Today'}
            </button>
          </div>

          <div className="card burn-card">
            <h2>Burn Tokens</h2>
            <p>Send tokens to someone (tokens are destroyed, recipient sees event) - FREE!</p>
            <div className="form-group">
              <label>Recipient Address:</label>
              <input
                type="text"
                value={burnRecipient}
                onChange={(e) => setBurnRecipient(e.target.value)}
                placeholder="5GrwvaEF5zXb26Fz..."
              />
            </div>
            <div className="form-group">
              <label>Amount (NST):</label>
              <input
                type="number"
                value={burnAmount}
                onChange={(e) => setBurnAmount(e.target.value)}
                placeholder="10"
                min="1"
              />
            </div>
            <button 
              onClick={burnTokens} 
              disabled={!burnRecipient || !burnAmount || loading}
              className="action-btn burn-btn"
            >
              {loading ? 'Processing...' : 'Burn Tokens'}
            </button>
          </div>

          {reputation && (
            <div className="card reputation-card">
              <h2>Your Reputation</h2>
              <div className="reputation-score">
                <span className="score-value">{calculateReputationScore(reputation)}</span>
                <span className="score-label">{getReputationLabel(calculateReputationScore(reputation))}</span>
              </div>
              <div className="rep-grid">
                <div className="rep-item">
                  <span className="rep-value">{formatTokens(reputation.totalBurned)}</span>
                  <span className="rep-label">Total Burned</span>
                </div>
                <div className="rep-item">
                  <span className="rep-value">{formatTokens(reputation.totalReceived)}</span>
                  <span className="rep-label">Total Received</span>
                </div>
                <div className="rep-item">
                  <span className="rep-value">{reputation.burnCount}</span>
                  <span className="rep-label">Burns Made</span>
                </div>
                <div className="rep-item">
                  <span className="rep-value">{reputation.receiveCount}</span>
                  <span className="rep-label">Burns Received</span>
                </div>
                <div className="rep-item">
                  <span className="rep-value">#{reputation.firstActivity || 'N/A'}</span>
                  <span className="rep-label">First Activity Block</span>
                </div>
              </div>
            </div>
          )}
        </>
      )}

      {status && (
        <div className="status-message">
          {status}
        </div>
      )}

      {/* Blockchain Explorer Section */}
      <div className="explorer-section">
        <div className="explorer-header" onClick={() => setShowExplorer(!showExplorer)}>
          <h2>Blockchain Explorer</h2>
          <span className="toggle-icon">{showExplorer ? '−' : '+'}</span>
        </div>
        
        {showExplorer && (
          <>
            {/* Chain Stats */}
            <div className="chain-stats">
              <div className="stat-item">
                <span className="stat-value">{blockNumber}</span>
                <span className="stat-label">Current Block</span>
              </div>
              <div className="stat-item">
                <span className="stat-value">{chainStats.finalizedBlock}</span>
                <span className="stat-label">Finalized</span>
              </div>
              <div className="stat-item">
                <span className="stat-value">{formatTokens(chainStats.totalSupply)}</span>
                <span className="stat-label">Total Supply (NST)</span>
              </div>
              <div className="stat-item">
                <span className="stat-value">{chainStats.avgBlockTime}s</span>
                <span className="stat-label">Avg Block Time</span>
              </div>
            </div>

            {/* Recent Blocks - Visual Chain */}
            <div className="explorer-panel">
              <h3>Recent Blocks</h3>
              <div className="blocks-chain">
                {recentBlocks.length === 0 ? (
                  <div className="empty-state">Waiting for blocks...</div>
                ) : (
                  recentBlocks.slice().reverse().map((block, index, arr) => (
                    <div key={block.hash} className={`block-chain-item ${index === 0 ? 'oldest' : ''} ${index === arr.length - 1 ? 'newest' : ''}`}>
                      {index > 0 && <div className="chain-connector" />}
                      <div 
                        className={`block-cube ${index === arr.length - 1 ? 'latest' : ''}`}
                        onClick={() => navigate(`/block/${block.number}`)}
                      >
                        <div className="cube-number">{block.number}</div>
                        <div className="cube-txs">{block.extrinsicsCount} tx</div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>

            {/* Recent Activity */}
            <div className="explorer-panel">
              <h3>Recent Activity</h3>
              <div className="events-list">
                {recentEvents.length === 0 ? (
                  <div className="empty-state">No recent UBI token activity</div>
                ) : (
                  recentEvents.map((event, i) => (
                    <div key={`${event.blockNumber}-${i}`} className="event-item">
                      <div className={`event-type ${event.method.toLowerCase()}`}>
                        {event.method}
                      </div>
                      <div className="event-details">
                        <span className="event-data">{formatEventData(event.method, event.data)}</span>
                        <span className="event-meta">Block #{event.blockNumber} · {formatTimeAgo(event.timestamp)}</span>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          </>
        )}
      </div>

      <footer>
        <p>NST - Non Speculative Tokens</p>
        <p className="small">Tokens cannot be transferred, only burned. No speculation possible. All transactions are FREE!</p>
      </footer>
    </div>
  );
}

export default App;
