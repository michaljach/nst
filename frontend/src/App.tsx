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
  weightedReceived: string;
  uniqueRecipientsCount: number;
  claimStreak: number;
  lastClaimPeriod: number;
  score: string;
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
  
  const [balance, setBalance] = useState<TokenBatch[]>([]);
  const [totalBalance, setTotalBalance] = useState<string>('0');
  const [claimableAmount, setClaimableAmount] = useState<string>('0');
  const [canClaim, setCanClaim] = useState(false);
  const [reputation, setReputation] = useState<Reputation | null>(null);
  
  const [burnRecipient, setBurnRecipient] = useState('');
  const [burnAmount, setBurnAmount] = useState('');
  
  const [status, setStatus] = useState('');
  const [loading, setLoading] = useState(false);

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

  const fetchHistoricalData = useCallback(async (api: ApiPromise, currentBlock: number) => {
    const blocks: BlockInfo[] = [];
    const events: ChainEvent[] = [];
    
    const startBlock = Math.max(1, currentBlock - 49);
    
    console.log(`Fetching blocks ${startBlock} to ${currentBlock}`);
    
    for (let blockNum = currentBlock; blockNum >= startBlock; blockNum--) {
      try {
        const blockHash = await api.rpc.chain.getBlockHash(blockNum);
        const signedBlock = await api.rpc.chain.getBlock(blockHash);
        const header = signedBlock.block.header;
        
        if (blocks.length < 8) {
          blocks.push({
            number: blockNum,
            hash: blockHash.toHex(),
            parentHash: header.parentHash.toHex(),
            extrinsicsCount: signedBlock.block.extrinsics.length,
            timestamp: Date.now() - ((currentBlock - blockNum) * 6000),
          });
        }
        
        const apiAt = await api.at(blockHash);
        const blockEvents = await apiAt.query.system.events();
        
        (blockEvents as unknown as Array<{ event: { section: string; method: string; data: { toString(): string } } }>).forEach((record) => {
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

  useEffect(() => {
    const connect = async () => {
      try {
        setStatus('Connecting to NST node...');
        const provider = new WsProvider('ws://127.0.0.1:9944');
        const api = await ApiPromise.create({ provider });
        setApi(api);
        setConnected(true);
        setStatus('Connected to NST node');
        
        const currentHeader = await api.rpc.chain.getHeader();
        const currentBlock = currentHeader.number.toNumber();
        setBlockNumber(currentBlock);
        
        await fetchHistoricalData(api, currentBlock);
        
        await api.rpc.chain.subscribeNewHeads(async (header) => {
          const blockNum = header.number.toNumber();
          setBlockNumber(blockNum);
          
          const now = Date.now();
          const timeDiff = (now - lastBlockTimeRef.current) / 1000;
          lastBlockTimeRef.current = now;
          
          blockTimesRef.current.push(timeDiff);
          if (blockTimesRef.current.length > 10) {
            blockTimesRef.current.shift();
          }
          const avgBlockTime = blockTimesRef.current.reduce((a, b) => a + b, 0) / blockTimesRef.current.length;
          
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
            if (prev.some(b => b.number === newBlock.number)) {
              return prev;
            }
            return [newBlock, ...prev].slice(0, 8);
          });
          
          const apiAt = await api.at(blockHash);
          const events = await apiAt.query.system.events();
          
          const blockEvents: ChainEvent[] = [];
          (events as unknown as Array<{ event: { section: string; method: string; data: { toString(): string } } }>).forEach((record) => {
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

  const fetchAccountData = useCallback(async () => {
    if (!api || !selectedAccount || blockNumber === 0) return;
    
    try {
      console.log('Fetching data for account:', selectedAccount, 'at block:', blockNumber);
      
      const balances = await api.query.ubiToken.balances(selectedAccount);
      const batchesRaw = balances.toJSON() as unknown[];
      console.log('Raw balances:', JSON.stringify(batchesRaw, null, 2));
      const batches: TokenBatch[] = (batchesRaw as Array<{ amount?: string; 0?: string; expiresAt?: number; expires_at?: number; 1?: number }>)?.map((b) => {
        console.log('Batch item:', b);
        return {
          amount: (b.amount ?? b[0] ?? '0').toString(),
          expiresAtBlock: b.expiresAt ?? b.expires_at ?? b[1] ?? 0,
        };
      }) || [];
      console.log('Parsed batches:', batches);
      setBalance(batches);
      
      const total = batches
        .filter(b => b.expiresAtBlock > blockNumber)
        .reduce((sum, b) => sum + BigInt(b.amount), BigInt(0));
      setTotalBalance(total.toString());
      
      const lastClaimBlock = await api.query.ubiToken.lastClaim(selectedAccount);
      const lastClaim = lastClaimBlock.toJSON() as number | null;
      console.log('Last claim block:', lastClaim);
      
      const claimPeriod = 10;
      
      if (lastClaim === null) {
        console.log('Never claimed - can claim now');
        setCanClaim(true);
        setClaimableAmount((100 * 10**9).toString());
      } else {
        const blocksSinceClaim = blockNumber - lastClaim;
        const periodsClaimable = Math.floor(blocksSinceClaim / claimPeriod);
        const canClaimNow = periodsClaimable > 0;
        console.log('Blocks since claim:', blocksSinceClaim, 'Periods claimable:', periodsClaimable);
        setCanClaim(canClaimNow);
        const periods = Math.min(periodsClaimable, 3);
        setClaimableAmount((periods * 100 * 10**9).toString());
      }
      
      const rep = await api.query.ubiToken.reputationStore(selectedAccount);
      const repJson = rep.toJSON() as {
        score?: number;
        weightedReceived?: number;
        weighted_received?: number;
        burnsSentVolume?: number;
        burns_sent_volume?: number;
        burnsReceivedVolume?: number;
        burns_received_volume?: number;
        burnsSentCount?: number;
        burns_sent_count?: number;
        burnsReceivedCount?: number;
        burns_received_count?: number;
        firstActivity?: number;
        first_activity?: number;
        uniqueRecipientsCount?: number;
        unique_recipients_count?: number;
        claimStreak?: number;
        claim_streak?: number;
        lastClaimPeriod?: number;
        last_claim_period?: number;
      } | null;
      console.log('Reputation raw:', JSON.stringify(repJson, null, 2));
      if (repJson) {
        const score = repJson.score ?? 0;
        const weightedReceived = repJson.weightedReceived ?? repJson.weighted_received ?? 0;
        
        setReputation({
          totalBurned: (repJson.burnsSentVolume ?? repJson.burns_sent_volume ?? 0).toString(),
          totalReceived: (repJson.burnsReceivedVolume ?? repJson.burns_received_volume ?? 0).toString(),
          burnCount: repJson.burnsSentCount ?? repJson.burns_sent_count ?? 0,
          receiveCount: repJson.burnsReceivedCount ?? repJson.burns_received_count ?? 0,
          firstActivity: repJson.firstActivity ?? repJson.first_activity ?? 0,
          weightedReceived: weightedReceived.toString(),
          uniqueRecipientsCount: repJson.uniqueRecipientsCount ?? repJson.unique_recipients_count ?? 0,
          claimStreak: repJson.claimStreak ?? repJson.claim_streak ?? 0,
          lastClaimPeriod: repJson.lastClaimPeriod ?? repJson.last_claim_period ?? 0,
          score: score.toString(),
        });
      }
    } catch (err) {
      console.error('Error fetching data:', err);
    }
  }, [api, selectedAccount, blockNumber]);

  useEffect(() => {
    fetchAccountData();
  }, [fetchAccountData]);

  const claimUBI = async () => {
    if (!api || !selectedAccount) return;
    
    setLoading(true);
    setStatus('Claiming UBI...');
    
    try {
      const tx = api.tx.ubiToken.claim(selectedAccount);
      console.log('TX hex:', tx.toHex());
      console.log('TX method:', tx.method.toHex());
      
      const hash = await api.rpc.author.submitExtrinsic(tx.toHex());
      console.log('Submitted hash:', hash.toHex());
      setStatus(`Claim submitted with hash: ${hash.toHex()}`);
      
      setTimeout(() => {
        fetchAccountData();
        setLoading(false);
        setStatus('Claim processed!');
      }, 6000);
    } catch (err: unknown) {
      console.error('Claim error:', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('Error message:', errorMessage);
      setStatus(`Claim failed: ${errorMessage}`);
      setLoading(false);
    }
  };

  const burnTokens = async () => {
    if (!api || !selectedAccount || !burnRecipient || !burnAmount) return;
    
    setLoading(true);
    setStatus('Burning tokens...');
    
    try {
      const amount = BigInt(burnAmount) * BigInt(10 ** 9);
      
      const tx = api.tx.ubiToken.burn(selectedAccount, burnRecipient, amount.toString());
      const extrinsic = api.createType('Extrinsic', tx);
      
      const hash = await api.rpc.author.submitExtrinsic(extrinsic);
      setStatus(`Burn submitted with hash: ${hash.toHex()}`);
      
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
    const whole = num / BigInt(10 ** 9);
    return whole.toString();
  };

  const getReputationScore = (rep: Reputation): number => {
    try {
      const scoreValue = rep.score;
      if (typeof scoreValue === 'string') {
        return Number(scoreValue) || 0;
      }
      return Number(scoreValue) || 0;
    } catch {
      return 0;
    }
  };

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

  const formatHash = (hash: string): string => {
    return `${hash.slice(0, 10)}...${hash.slice(-8)}`;
  };

  const formatTimeAgo = (timestamp: number): string => {
    const seconds = Math.floor((Date.now() - timestamp) / 1000);
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    return `${hours}h ago`;
  };

  const formatEventData = (method: string, data: string): string => {
    try {
      const parts = data.replace(/[[\]]/g, '').split(',').map(s => s.trim());
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
    <div className="app-container">
      <header className="app-header">
        <h1 className="app-title">NST Wallet</h1>
        <p className="app-subtitle">Non-Speculative Tokens • Burn-Only UBI</p>
      </header>

      <div className="status-bar">
        <div className="connection-status">
          <span className={`status-indicator ${connected ? 'connected' : ''}`} />
          <span>{connected ? 'Connected to Node' : 'Disconnected'}</span>
        </div>
        <span className="block-counter">Block #{blockNumber.toLocaleString()}</span>
      </div>

      {!selectedAccount ? (
        <div className="wallet-connect">
          <div className="wallet-icon">+</div>
          <button className="btn btn-primary btn-full" onClick={connectWallet}>
            Connect Wallet
          </button>
        </div>
      ) : (
        <>
          <div className="account-selector">
            <label>Account</label>
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

          <div className="dashboard-grid">
            <div className="card balance-card">
              <h2 className="card-title">Your Balance</h2>
              <div className="balance-display">
                <div className="balance-amount">{formatTokens(totalBalance)}</div>
                <span className="balance-unit">NST</span>
                <div className="balance-meta">
                  {balance.filter(b => b.expiresAtBlock > blockNumber).length} active batch(es)
                </div>
              </div>
              
              {balance.length > 0 && (
                <div className="batches-list">
                  <div className="batches-title">Token Batches</div>
                  {balance.map((batch, i) => (
                    <div 
                      key={i} 
                      className={`batch-item ${batch.expiresAtBlock <= blockNumber ? 'expired' : ''}`}
                    >
                      <span className="batch-amount">{formatTokens(batch.amount)} NST</span>
                      <span className="batch-expiry">
                        {batch.expiresAtBlock <= blockNumber 
                          ? 'EXPIRED' 
                          : `Expires #${batch.expiresAtBlock.toLocaleString()}`}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="card claim-card">
              <h2 className="card-title">Claim UBI</h2>
              <div className="claim-section">
                <p style={{ color: 'var(--text-muted)', marginBottom: '1.5rem' }}>
                  Receive 100 NST/day (expires in 7 days)
                </p>
                <div className="claim-info">
                  <span>Claimable:</span>
                  <span className="claim-amount">{formatTokens(claimableAmount)} NST</span>
                </div>
                <button 
                  onClick={claimUBI} 
                  disabled={!canClaim || loading}
                  className="btn btn-primary btn-full"
                >
                  {loading ? 'Processing...' : canClaim ? 'Claim Now' : 'Already Claimed Today'}
                </button>
              </div>
            </div>

            <div className="card burn-card">
              <h2 className="card-title">Burn Tokens</h2>
              <p style={{ color: 'var(--text-muted)', marginBottom: '1.5rem' }}>
                Send tokens to someone (tokens are destroyed, recipient sees event)
              </p>
              <div className="form-group">
                <label className="form-label">Recipient Address</label>
                <input
                  type="text"
                  className="form-input"
                  value={burnRecipient}
                  onChange={(e) => setBurnRecipient(e.target.value)}
                  placeholder="5GrwvaEF5zXb26Fz..."
                />
              </div>
              <div className="form-group">
                <label className="form-label">Amount (NST)</label>
                <input
                  type="number"
                  className="form-input"
                  value={burnAmount}
                  onChange={(e) => setBurnAmount(e.target.value)}
                  placeholder="10"
                  min="1"
                />
              </div>
              <button 
                onClick={burnTokens} 
                disabled={!burnRecipient || !burnAmount || loading}
                className="btn btn-danger btn-full"
              >
                {loading ? 'Processing...' : 'Burn Tokens'}
              </button>
            </div>

            {reputation && (
              <div className="card reputation-card">
                <h2 className="card-title">Your Reputation</h2>
                <div className="reputation-header">
                  <div className="reputation-score">{getReputationScore(reputation)}</div>
                  <div className="reputation-label">{getReputationLabel(getReputationScore(reputation))}</div>
                </div>
                <div className="reputation-grid">
                  <div className="reputation-stat">
                    <span className="stat-value">{reputation.claimStreak}</span>
                    <span className="stat-name">Claim Streak</span>
                  </div>
                  <div className="reputation-stat">
                    <span className="stat-value">{reputation.uniqueRecipientsCount}</span>
                    <span className="stat-name">Unique Recipients</span>
                  </div>
                  <div className="reputation-stat">
                    <span className="stat-value">{formatTokens(reputation.totalBurned)}</span>
                    <span className="stat-name">Total Burned</span>
                  </div>
                  <div className="reputation-stat">
                    <span className="stat-value">{formatTokens(reputation.weightedReceived)}</span>
                    <span className="stat-name">Weighted Received</span>
                  </div>
                  <div className="reputation-stat">
                    <span className="stat-value">{reputation.burnCount}</span>
                    <span className="stat-name">Burns Made</span>
                  </div>
                  <div className="reputation-stat">
                    <span className="stat-value">{reputation.receiveCount}</span>
                    <span className="stat-name">Burns Received</span>
                  </div>
                </div>
              </div>
            )}
          </div>
        </>
      )}

      {status && (
        <div className="status-message">
          {status}
        </div>
      )}

      <div className="explorer-card">
        <div className="explorer-header" onClick={() => setShowExplorer(!showExplorer)}>
          <h2 className="explorer-title">Blockchain Explorer</h2>
          <span className={`explorer-toggle ${showExplorer ? 'open' : ''}`}>+</span>
        </div>
        
        {showExplorer && (
          <>
            <div className="chain-stats">
              <div className="chain-stat">
                <span className="chain-stat-value">{blockNumber.toLocaleString()}</span>
                <span className="chain-stat-label">Current Block</span>
              </div>
              <div className="chain-stat">
                <span className="chain-stat-value">{chainStats.finalizedBlock.toLocaleString()}</span>
                <span className="chain-stat-label">Finalized</span>
              </div>
              <div className="chain-stat">
                <span className="chain-stat-value">{formatTokens(chainStats.totalSupply)}</span>
                <span className="chain-stat-label">Total Supply (NST)</span>
              </div>
              <div className="chain-stat">
                <span className="chain-stat-value">{chainStats.avgBlockTime}s</span>
                <span className="chain-stat-label">Avg Block Time</span>
              </div>
            </div>

            <div className="blocks-visual">
              <div className="blocks-chain">
                {recentBlocks.length === 0 ? (
                  <div className="empty-state">
                    <div className="empty-state-icon">—</div>
                    <div>Waiting for blocks...</div>
                  </div>
                ) : (
                  recentBlocks.slice().reverse().map((block, index, arr) => (
                    <div key={block.hash} className="block-node">
                      {index > 0 && <div className="chain-link" />}
                      <div 
                        className={`block-cube ${index === arr.length - 1 ? 'latest' : ''}`}
                        onClick={() => navigate(`/block/${block.number}`)}
                      >
                        <div className="block-number">#{block.number.toLocaleString()}</div>
                        <div className="block-txs">{block.extrinsicsCount} tx</div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="events-panel">
              <div className="events-title">Recent Activity</div>
              <div className="events-list">
                {recentEvents.length === 0 ? (
                  <div className="empty-state">
                    <div className="empty-state-icon">—</div>
                    <div>No recent UBI token activity</div>
                  </div>
                ) : (
                  recentEvents.map((event, i) => (
                    <div key={`${event.blockNumber}-${i}`} className="event-item">
                      <div className={`event-badge ${event.method.toLowerCase()}`}>
                        {event.method}
                      </div>
                      <div className="event-content">
                        <div className="event-description">{formatEventData(event.method, event.data)}</div>
                        <div className="event-meta">
                          Block #{event.blockNumber.toLocaleString()} • {formatTimeAgo(event.timestamp)}
                        </div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          </>
        )}
      </div>

      <footer className="app-footer">
        <p className="footer-text">NST - Non Speculative Tokens</p>
        <p className="footer-tagline">Tokens cannot be transferred, only burned. No speculation possible.</p>
      </footer>
    </div>
  );
}

export default App;
