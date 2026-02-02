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

  useEffect(() => {
    const connect = async () => {
      try {
        const provider = new WsProvider('ws://127.0.0.1:9944');
        const api = await ApiPromise.create({ provider });
        setApi(api);
      } catch {
        setError('Failed to connect to node');
        setLoading(false);
      }
    };
    connect();
  }, []);

  const fetchBlock = useCallback(async () => {
    if (!api || !blockId) return;
    
    setLoading(true);
    setError(null);
    
    try {
      let blockHash: string;
      
      if (/^\d+$/.test(blockId)) {
        const hash = await api.rpc.chain.getBlockHash(parseInt(blockId));
        blockHash = hash.toHex();
      } else if (blockId.startsWith('0x')) {
        blockHash = blockId;
      } else {
        setError('Invalid block identifier. Use block number or hash (0x...)');
        setLoading(false);
        return;
      }
      
      const signedBlock = await api.rpc.chain.getBlock(blockHash);
      const header = signedBlock.block.header;
      
      let timestamp: number | null = null;
      (signedBlock.block.extrinsics as unknown as Array<{ method: { section: string; method: string; args: Array<{ toString(): string }> } }>).forEach((ext) => {
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
      
      const exts: Extrinsic[] = (signedBlock.block.extrinsics as unknown as Array<{ method: { section: string; method: string; args: Array<{ toString(): string }> }; isSigned: boolean; signer: { toString(): string } }>).map((ext, index: number) => {
        const { method, section } = ext.method;
        const signer = ext.isSigned ? ext.signer.toString() : null;
        const args = ext.method.args.map((arg) => arg.toString()).join(', ');
        
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
    <div className="block-details-container">
      <div className="details-header">
        <button className="back-btn" onClick={() => navigate('/')}>
          <span>←</span>
          <span>Back</span>
        </button>
        <h1 className="details-title">Block Details</h1>
      </div>

      {loading ? (
        <div className="loading-state">
          <div className="loading-spinner" />
          <div>Loading block data...</div>
        </div>
      ) : error ? (
        <div className="error-state">
          <div style={{ fontSize: '2rem', marginBottom: '1rem', color: 'var(--text-muted)' }}>×</div>
          <div>{error}</div>
        </div>
      ) : block ? (
        <>
          <div className="info-card">
            <h2 className="info-card-title">Block #{block.number.toLocaleString()}</h2>
            
            <div className="info-row">
              <span className="info-label">Block Hash</span>
              <span className="info-value mono">{block.hash}</span>
            </div>
            
            <div className="info-row">
              <span className="info-label">Parent Hash</span>
              <span 
                className="info-value mono clickable" 
                onClick={() => navigate(`/block/${block.number - 1}`)}
              >
                {block.parentHash}
              </span>
            </div>
            
            <div className="info-row">
              <span className="info-label">State Root</span>
              <span className="info-value mono">{block.stateRoot}</span>
            </div>
            
            <div className="info-row">
              <span className="info-label">Extrinsics Root</span>
              <span className="info-value mono">{block.extrinsicsRoot}</span>
            </div>
            
            {block.timestamp && (
              <div className="info-row">
                <span className="info-label">Timestamp</span>
                <span className="info-value">
                  {new Date(block.timestamp).toLocaleString()} ({formatTimeAgo(block.timestamp)})
                </span>
              </div>
            )}
          </div>

          <div className="info-card">
            <h2 className="info-card-title">Extrinsics ({extrinsics.length})</h2>
            
            {extrinsics.length === 0 ? (
              <div className="empty-state">
                <div className="empty-state-icon">—</div>
                <div>No extrinsics in this block</div>
              </div>
            ) : (
              <div className="extrinsics-list">
                {extrinsics.map((ext) => (
                  <div key={ext.index} className="extrinsic-item">
                    <div className="extrinsic-header">
                      <span className="extrinsic-index">#{ext.index}</span>
                      <span className="extrinsic-method">{ext.section}.{ext.method}</span>
                      {ext.signer ? (
                        <span className="extrinsic-badge signed">Signed</span>
                      ) : (
                        <span className="extrinsic-badge unsigned">Unsigned</span>
                      )}
                    </div>
                    
                    {ext.signer && (
                      <div className="extrinsic-detail">
                        <span className="extrinsic-detail-label">Signer</span>
                        <span className="extrinsic-detail-value">{ext.signer}</span>
                      </div>
                    )}
                    
                    {ext.args && (
                      <div className="extrinsic-detail">
                        <span className="extrinsic-detail-label">Arguments</span>
                        <span className="extrinsic-detail-value">{ext.args || '(none)'}</span>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="details-nav">
            {block.number > 1 && (
              <button className="nav-btn" onClick={() => navigate(`/block/${block.number - 1}`)}>
                <span>←</span>
                <span>Block #{block.number - 1}</span>
              </button>
            )}
            <button className="nav-btn" onClick={() => navigate(`/block/${block.number + 1}`)}>
              <span>Block #{block.number + 1}</span>
              <span>→</span>
            </button>
          </div>
        </>
      ) : null}
    </div>
  );
}

export default BlockDetails;
