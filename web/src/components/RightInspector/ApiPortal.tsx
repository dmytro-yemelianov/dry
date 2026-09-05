import React, { useState } from 'react';

export const ApiPortal: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'mcp' | 'rest'>('mcp');
  const [testResponse, setTestResponse] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const testApi = async (endpoint: string) => {
    setIsLoading(true);
    setTestResponse(null);
    try {
      const res = await fetch(endpoint);
      const data = await res.json();
      setTestResponse(JSON.stringify(data, null, 2));
    } catch (err: any) {
      setTestResponse(`Error: ${err.message}`);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="api-portal-root" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      {/* Tab Switcher */}
      <div className="optimizer-card">
        <div className="opt-card-title">Local MCP & Pages Catalog APIs</div>
        <div className="opt-mode-pills">
          <button
            className={`opt-mode-btn ${activeTab === 'mcp' ? 'active' : ''}`}
            onClick={() => setActiveTab('mcp')}
          >
            <span style={{ fontWeight: 700 }}>Local MCP Server</span>
          </button>
          <button
            className={`opt-mode-btn ${activeTab === 'rest' ? 'active' : ''}`}
            onClick={() => setActiveTab('rest')}
          >
            <span style={{ fontWeight: 700 }}>REST API Endpoints</span>
          </button>
        </div>
      </div>

      {activeTab === 'mcp' ? (
        <>
          {/* MCP Server Overview */}
          <div className="optimizer-card">
            <div className="opt-card-title">🤖 Local Model Context Protocol (MCP)</div>
            <div style={{ fontSize: '11px', color: 'var(--fg-muted)', marginBottom: '8px' }}>
              Run the published <code>@dry/mcp</code> package on your machine. The former
              unauthenticated Pages MCP endpoint is retired and is not a hosted verification tier.
            </div>

            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px' }}>
                <span style={{ fontSize: '10.5px', fontWeight: 700, color: 'var(--accent)' }}>Claude Desktop Config</span>
                <button
                  className="param-reset-btn"
                  onClick={() => navigator.clipboard?.writeText(JSON.stringify({ mcpServers: { "dry-machina": { command: "npx", args: ["-y", "@dry/mcp"] } } }, null, 2))}
                >
                  Copy
                </button>
              </div>
              <pre
                style={{
                  background: 'var(--bg-app)',
                  border: '1px solid var(--border)',
                  borderRadius: '4px',
                  padding: '6px 8px',
                  fontSize: '10px',
                  fontFamily: 'ui-monospace, monospace',
                  color: 'var(--fg-bright)',
                }}
              >
{`{
  "mcpServers": {
    "dry-machina": {
      "command": "npx",
      "args": ["-y", "@dry/mcp"]
    }
  }
}`}
              </pre>
            </div>
          </div>
        </>
      ) : (
        <>
          {/* REST API Endpoints */}
          <div className="optimizer-card">
            <div className="opt-card-title">Pages Catalog Endpoints</div>
            <div style={{ fontSize: '11px', color: 'var(--fg-muted)', marginBottom: '8px' }}>
              These catalog helpers do not verify toolpaths. Hosted verification uses the separate,
              authenticated asynchronous <code>POST /v1/jobs/verify</code> contract; no live public
              origin is claimed until deployment and launch gates are complete.
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <div>
                  <span className="reduction-chip" style={{ background: '#238636', marginLeft: 0, marginRight: '6px' }}>GET</span>
                  <code style={{ fontSize: '11px', color: 'var(--fg-bright)' }}>/api/macros</code>
                </div>
                <button className="param-reset-btn" onClick={() => testApi('/api/macros')}>
                  Test
                </button>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <div>
                  <span className="reduction-chip" style={{ background: '#238636', marginLeft: 0, marginRight: '6px' }}>GET</span>
                  <code style={{ fontSize: '11px', color: 'var(--fg-bright)' }}>/api/machines</code>
                </div>
                <button className="param-reset-btn" onClick={() => testApi('/api/machines')}>
                  Test
                </button>
              </div>
            </div>
          </div>

          {testResponse && (
            <div className="optimizer-card">
              <div className="opt-card-title">Live Response Output</div>
              <pre
                style={{
                  background: 'var(--bg-app)',
                  border: '1px solid var(--border)',
                  borderRadius: '4px',
                  padding: '8px',
                  fontSize: '10px',
                  fontFamily: 'ui-monospace, monospace',
                  color: 'var(--accent)',
                  maxHeight: '140px',
                  overflowY: 'auto',
                }}
              >
                {testResponse}
              </pre>
            </div>
          )}
        </>
      )}
    </div>
  );
};
