import { useState, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ApiPromise, WsProvider } from '@polkadot/api';

interface Extrinsic {
  index: number;
  section: string;
  method: string;
  args: string;
  signer: string | null;
}

interface BlockData {
  number: number;
  hash: string;
  parentHash: string;
  stateRoot: string;
  extrinsicsRoot: string;
  extrinsicsCount: number;
  timestamp: number | null;
}

function formatTimeAgo(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function BlockDetails() {
  const { blockId } = useParams<{ blockId: string }>();
  const navigate = useNavigate();
  
  const [api, setApi] = useState<ApiPromise | null>(null);
  const [block, setBlock] = useState<BlockData | null>(null);
  const [extrinsics, setExtrinsics] = useState<Extrinsic[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Connect to node
  useEffect(() => {
    const connect = async () => {
      try {
        const provider = new WsProvider('ws://127.0.0.1:9944');
        const api = await ApiPromise.create({ provider });
        setApi(api);
      } catch (e) {
        setError('Failed to connect to node');
        setLoading(false);
      }
    };
    connect();
  }, []);

  // Fetch block data
  const fetchBlock = useCallback(async () => {
    if (!api || !blockId) return;
    
    setLoading(true);
    setError(null);
    
    try {
      let blockHash: string;
      
      // Check if blockId is a number or hash
      if (/^\d+$/.test(blockId)) {
        // It's a block number
        const hash = await api.rpc.chain.getBlockHash(parseInt(blockId));
        blockHash = hash.toHex();
      } else if (blockId.startsWith('0x')) {
        // It's a hash
        blockHash = blockId;
      } else {
        setError('Invalid block identifier. Use block number or hash (0x...)');
        setLoading(false);
        return;
      }
      
      const signedBlock = await api.rpc.chain.getBlock(blockHash);
      const header = signedBlock.block.header;
      
      // Get timestamp from extrinsics if available
      let timestamp: number | null = null;
      signedBlock.block.extrinsics.forEach((ext: any) => {
        if (ext.method.section === 'timestamp' && ext.method.method === 'set') {
          timestamp = parseInt(ext.method.args[0].toString());
        }
      });
      
      const blockData: BlockData = {
        number: header.number.toNumber(),
        hash: blockHash,
        parentHash: header.parentHash.toHex(),
        stateRoot: header.stateRoot.toHex(),
        extrinsicsRoot: header.extrinsicsRoot.toHex(),
        extrinsicsCount: signedBlock.block.extrinsics.length,
        timestamp,
      };
      
      setBlock(blockData);
      
      // Parse extrinsics
      const exts: Extrinsic[] = signedBlock.block.extrinsics.map((ext: any, index: number) => {
        const { method, section } = ext.method;
        const signer = ext.isSigned ? ext.signer.toString() : null;
        const args = ext.method.args.map((arg: any) => arg.toString()).join(', ');
        
        return {
          index,
          section,
          method,
          args,
          signer,
        };
      });
      
      setExtrinsics(exts);
    } catch (e) {
      console.error('Error fetching block:', e);
      setError('Block not found');
    } finally {
      setLoading(false);
    }
  }, [api, blockId]);

  useEffect(() => {
    if (api) {
      fetchBlock();
    }
  }, [api, fetchBlock]);

  return (
    <div className="block-details-page">
      <header className="block-header">
        <button className="back-button" onClick={() => navigate('/')}>
          &larr; Back
        </button>
        <h1>Block Details</h1>
      </header>

      <main className="block-main">
        {loading ? (
          <div className="loading-state">Loading block data...</div>
        ) : error ? (
          <div className="error-state">{error}</div>
        ) : block ? (
          <>
            <div className="block-info-card">
              <h2>Block #{block.number}</h2>
              
              <div className="info-grid">
                <div className="info-item">
                  <span className="info-label">Block Hash</span>
                  <span className="info-value mono">{block.hash}</span>
                </div>
                <div className="info-item">
                  <span className="info-label">Parent Hash</span>
                  <span className="info-value mono clickable" onClick={() => navigate(`/block/${block.number - 1}`)}>
                    {block.parentHash}
                  </span>
                </div>
                <div className="info-item">
                  <span className="info-label">State Root</span>
                  <span className="info-value mono">{block.stateRoot}</span>
                </div>
                <div className="info-item">
                  <span className="info-label">Extrinsics Root</span>
                  <span className="info-value mono">{block.extrinsicsRoot}</span>
                </div>
                {block.timestamp && (
                  <div className="info-item">
                    <span className="info-label">Timestamp</span>
                    <span className="info-value">
                      {new Date(block.timestamp).toLocaleString()} ({formatTimeAgo(block.timestamp)})
                    </span>
                  </div>
                )}
              </div>
            </div>

            <div className="block-info-card">
              <h2>Extrinsics ({extrinsics.length})</h2>
              
              {extrinsics.length === 0 ? (
                <div className="empty-state">No extrinsics in this block</div>
              ) : (
                <div className="extrinsics-grid">
                  {extrinsics.map((ext) => (
                    <div key={ext.index} className="extrinsic-card">
                      <div className="ext-header">
                        <span className="ext-index">#{ext.index}</span>
                        <span className="ext-call">{ext.section}.{ext.method}</span>
                        {ext.signer ? (
                          <span className="ext-signed">Signed</span>
                        ) : (
                          <span className="ext-unsigned">Unsigned</span>
                        )}
                      </div>
                      
                      {ext.signer && (
                        <div className="ext-detail">
                          <span className="ext-label">Signer</span>
                          <span className="ext-value mono">{ext.signer}</span>
                        </div>
                      )}
                      
                      {ext.args && (
                        <div className="ext-detail">
                          <span className="ext-label">Arguments</span>
                          <span className="ext-value mono">{ext.args || '(none)'}</span>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Navigation */}
            <div className="block-navigation">
              {block.number > 1 && (
                <button onClick={() => navigate(`/block/${block.number - 1}`)}>
                  &larr; Block #{block.number - 1}
                </button>
              )}
              <button onClick={() => navigate(`/block/${block.number + 1}`)}>
                Block #{block.number + 1} &rarr;
              </button>
            </div>
          </>
        ) : null}
      </main>
    </div>
  );
}

export default BlockDetails;
