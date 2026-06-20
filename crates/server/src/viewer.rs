/// Single source of truth: the viewer page shipped at `viewer/index.html`.
/// (The HTTP server serves that file directly via `include_str!`.)
pub const VIEWER_HTML: &str = include_str!("../../../viewer/index.html");

#[allow(dead_code)]
const _LEGACY_WEBRTC_VIEWER: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Screen Stream</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { 
            background: #1a1a1a; 
            color: #fff; 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
            display: flex; 
            flex-direction: column;
            align-items: center; 
            justify-content: center;
            min-height: 100vh;
        }
        #container { 
            position: relative;
            max-width: 100vw;
            max-height: 100vh;
        }
        video { 
            max-width: 100vw;
            max-height: 100vh;
            object-fit: contain;
        }
        #status {
            position: fixed;
            top: 20px;
            left: 20px;
            padding: 10px 20px;
            background: rgba(0,0,0,0.7);
            border-radius: 8px;
            font-size: 14px;
        }
        #auth-overlay {
            position: fixed;
            top: 0; left: 0; right: 0; bottom: 0;
            background: rgba(0,0,0,0.9);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 100;
        }
        #auth-box {
            background: #2a2a2a;
            padding: 40px;
            border-radius: 12px;
            text-align: center;
        }
        #auth-box h2 { margin-bottom: 20px; }
        #auth-box input {
            padding: 12px 20px;
            font-size: 16px;
            border: 1px solid #444;
            border-radius: 8px;
            background: #1a1a1a;
            color: #fff;
            width: 300px;
            margin-bottom: 20px;
        }
        #auth-box button {
            padding: 12px 30px;
            font-size: 16px;
            border: none;
            border-radius: 8px;
            background: #4a9eff;
            color: #fff;
            cursor: pointer;
        }
        #auth-box button:hover { background: #3a8eef; }
    </style>
</head>
<body>
    <div id="status">Connecting...</div>
    <div id="auth-overlay">
        <div id="auth-box">
            <h2>Screen Stream</h2>
            <input type="password" id="token-input" placeholder="Enter access token">
            <br>
            <button onclick="authenticate()">Connect</button>
        </div>
    </div>
    <div id="container">
        <video id="video" autoplay playsinline></video>
    </div>

    <script>
        let pc = null;
        let ws = null;

        function authenticate() {
            const token = document.getElementById('token-input').value;
            if (!token) return;
            
            fetch('/api/token', { method: 'POST' })
                .then(r => r.json())
                .then(data => {
                    connectWebSocket(data.token || token);
                })
                .catch(() => {
                    connectWebSocket(token);
                });
        }

        function connectWebSocket(token) {
            const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${location.host}/ws`);
            
            ws.onopen = () => {
                document.getElementById('status').textContent = 'Authenticating...';
                ws.send(JSON.stringify({ type: 'auth', token: token }));
            };

            ws.onmessage = (event) => {
                const msg = JSON.parse(event.data);
                
                switch(msg.type) {
                    case 'auth_ok':
                        document.getElementById('auth-overlay').style.display = 'none';
                        document.getElementById('status').textContent = 'Connected - Waiting for stream...';
                        startWebRTC();
                        break;
                    case 'auth_error':
                        document.getElementById('status').textContent = 'Auth failed: ' + msg.error;
                        break;
                    case 'answer':
                        if (pc) {
                            pc.setRemoteDescription(new RTCSessionDescription({
                                type: 'answer',
                                sdp: msg.sdp
                            }));
                        }
                        break;
                    case 'ice':
                        if (pc) {
                            pc.addIceCandidate(new RTCIceCandidate({
                                candidate: msg.candidate,
                                sdpMid: msg.sdp_mid,
                                sdpMLineIndex: msg.sdp_m_line_index
                            }));
                        }
                        break;
                }
            };

            ws.onclose = () => {
                document.getElementById('status').textContent = 'Disconnected';
                setTimeout(() => connectWebSocket(token), 3000);
            };
        }

        function startWebRTC() {
            const config = {
                iceServers: [
                    { urls: 'stun:stun.l.google.com:19302' },
                    { urls: 'stun:stun1.l.google.com:19302' }
                ]
            };

            pc = new RTCPeerConnection(config);
            
            pc.ontrack = (event) => {
                const video = document.getElementById('video');
                video.srcObject = event.streams[0];
                document.getElementById('status').textContent = 'Streaming';
            };

            pc.onicecandidate = (event) => {
                if (event.candidate) {
                    ws.send(JSON.stringify({
                        type: 'ice',
                        candidate: event.candidate.candidate,
                        sdp_mid: event.candidate.sdpMid,
                        sdp_m_line_index: event.candidate.sdpMLineIndex
                    }));
                }
            };

            pc.onconnectionstatechange = () => {
                if (pc.connectionState === 'disconnected' || pc.connectionState === 'failed') {
                    document.getElementById('status').textContent = 'Connection lost';
                }
            };

            const transceiver = pc.addTransceiver('video', { direction: 'recvonly' });
            
            pc.createOffer().then(offer => {
                return pc.setLocalDescription(offer);
            }).then(() => {
                ws.send(JSON.stringify({
                    type: 'offer',
                    sdp: pc.localDescription.sdp,
                    sdp_type: pc.localDescription.type
                }));
            });
        }

        document.getElementById('token-input').addEventListener('keypress', (e) => {
            if (e.key === 'Enter') authenticate();
        });
    </script>
</body>
</html>"#;
