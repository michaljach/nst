import { useState, useEffect, useCallback } from 'react';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { web3Accounts, web3Enable, web3FromAddress } from '@polkadot/extension-dapp';
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

function App() {
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
        
        // Subscribe to new blocks
        await api.rpc.chain.subscribeNewHeads((header) => {
          setBlockNumber(header.number.toNumber());
        });
      } catch (err) {
        setStatus(`Failed to connect: ${err}`);
      }
    };
    connect();
  }, []);

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
    if (!api || !selectedAccount) return;
    
    try {
      // Get token batches
      const balances = await api.query.ubiToken.balances(selectedAccount);
      const batchesRaw = balances.toJSON() as any[];
      const batches: TokenBatch[] = batchesRaw?.map((b: any) => ({
        amount: b.amount?.toString() || '0',
        expiresAtBlock: b.expiresAtBlock || 0,
      })) || [];
      setBalance(batches);
      
      // Calculate total valid balance
      const total = batches
        .filter(b => b.expiresAtBlock > blockNumber)
        .reduce((sum, b) => sum + BigInt(b.amount), BigInt(0));
      setTotalBalance(total.toString());
      
      // Check if can claim
      const canClaimResult = await api.call.ubiTokenApi?.canClaim?.(selectedAccount);
      setCanClaim((canClaimResult as any)?.isTrue || canClaimResult?.toJSON() === true || false);
      
      // Get claimable amount
      const claimable = await api.call.ubiTokenApi?.claimableAmount?.(selectedAccount);
      setClaimableAmount(claimable?.toString() || '0');
      
      // Get reputation
      const rep = await api.query.ubiToken.reputationStore(selectedAccount);
      const repJson = rep.toJSON() as any;
      if (repJson) {
        setReputation({
          totalBurned: repJson.totalBurned?.toString() || '0',
          totalReceived: repJson.totalReceived?.toString() || '0',
          burnCount: repJson.burnCount || 0,
          receiveCount: repJson.receiveCount || 0,
          firstActivity: repJson.firstActivity || 0,
        });
      }
    } catch (err) {
      console.error('Error fetching data:', err);
    }
  }, [api, selectedAccount, blockNumber]);

  useEffect(() => {
    fetchAccountData();
  }, [fetchAccountData]);

  // Claim UBI
  const claimUBI = async () => {
    if (!api || !selectedAccount) return;
    
    setLoading(true);
    setStatus('Claiming UBI...');
    
    try {
      const injector = await web3FromAddress(selectedAccount);
      
      await api.tx.ubiToken
        .claim()
        .signAndSend(selectedAccount, { signer: injector.signer }, ({ status }) => {
          if (status.isInBlock) {
            setStatus(`Claim included in block ${status.asInBlock.toHex()}`);
            fetchAccountData();
          } else if (status.isFinalized) {
            setStatus('Claim finalized!');
            setLoading(false);
          }
        });
    } catch (err) {
      setStatus(`Claim failed: ${err}`);
      setLoading(false);
    }
  };

  // Burn tokens
  const burnTokens = async () => {
    if (!api || !selectedAccount || !burnRecipient || !burnAmount) return;
    
    setLoading(true);
    setStatus('Burning tokens...');
    
    try {
      const injector = await web3FromAddress(selectedAccount);
      const amount = BigInt(burnAmount) * BigInt(10 ** 12); // 12 decimals
      
      await api.tx.ubiToken
        .burn(burnRecipient, amount.toString())
        .signAndSend(selectedAccount, { signer: injector.signer }, ({ status, events }) => {
          if (status.isInBlock) {
            setStatus(`Burn included in block ${status.asInBlock.toHex()}`);
            
            // Check for events
            events.forEach(({ event }) => {
              if (api.events.ubiToken.TokensBurned.is(event)) {
                const [_sender, recipient, burnedAmount] = event.data;
                setStatus(`Burned ${burnedAmount.toString()} tokens to ${recipient.toString()}`);
              }
            });
            
            fetchAccountData();
            setBurnRecipient('');
            setBurnAmount('');
          } else if (status.isFinalized) {
            setStatus('Burn finalized!');
            setLoading(false);
          }
        });
    } catch (err) {
      setStatus(`Burn failed: ${err}`);
      setLoading(false);
    }
  };

  const formatTokens = (amount: string) => {
    const num = BigInt(amount);
    const whole = num / BigInt(10 ** 12);
    return whole.toString();
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
            <p>Receive 100 NST/day (expires in 7 days)</p>
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
            <p>Send tokens to someone (tokens are destroyed, recipient sees event)</p>
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

      <footer>
        <p>NST - Non Speculative Tokens</p>
        <p className="small">Tokens cannot be transferred, only burned. No speculation possible.</p>
      </footer>
    </div>
  );
}

export default App;
