import { createHmac } from "node:crypto";


interface Peer {
  name: string;
  socket: WebSocket;
  type: 'user' | 'server';
  candidates: string[];
  connectedTo?: string;
}

const peers = new Map<string, Peer>();

function generateTurnCredentials(usernameBase: string, ttl: number = 86400): { username: string; password: string } {
  const secret = Deno.env.get("COTURN_SHARED_SECRET");
  if (!secret) throw new Error("Shared secret not set in environment variables.");

  const unixTimeStamp = Math.floor(Date.now() / 1000) + ttl;
  const username = `${unixTimeStamp}:${usernameBase}`;
  const password = createHmac("sha1", secret).update(username).digest("base64");

  return { username, password };
}


function handleWs(sock: WebSocket) {
  console.log('WebSocket connection established');

  let peerName = '';
  let peerType: 'user' | 'server' = 'user';

  try {
    sock.onmessage = (ev) => {
      try {
        const data = ev.data;


        const message = JSON.parse(data);
        if (!message) return;

        console.log(message);
        switch (message.type) {
          case 'register': {
            peerName = message.name;
            peerType = message.peer_type;
            if (peers.has(peerName)) {
              // Name already taken
              sock.send(
                JSON.stringify({ type: 'error', message: 'Name already taken' })
              );
              sock.close();
              return;
            }
            peers.set(peerName, {
              name: peerName,
              socket: sock,
              type: peerType,
              candidates: [],
            });
            console.log(`Peer registered: ${peerName} (${peerType})`);

            // Send TURN credentials to the newly registered peer
            // const turnCredentials = generateTurnCredentials(peerName);
            // sock.send(JSON.stringify({ type: 'turn_credentials', data: JSON.stringify(turnCredentials) }));
            console.log(`Peer registered: ${peerName} (${peerType})`);

            break;
          }
          case 'connect': {
            // User wants to connect to a server
            const targetName = message.target;
            if (!peers.has(targetName)) {
              sock.send(
                JSON.stringify({ type: 'error', message: 'Target not found' })
              );
              break;
            }
            const targetPeer = peers.get(targetName)!;
            // Save connection information
            peers.get(peerName)!.connectedTo = targetName;
            targetPeer.connectedTo = peerName;
            console.log(`${peerName} is connecting to ${targetName}`);

            // Notify target peer
            targetPeer.socket.send(
              JSON.stringify({ type: 'connection_request', from: peerName })
            );
            break;
          }
          case 'offer':
          case 'answer':
          case 'signal': {
            // Forward signaling messages to the connected peer
            const connectedPeerName = message.target;
            if (!connectedPeerName || !peers.has(connectedPeerName)) {
              sock.send(
                JSON.stringify({
                  type: 'error',
                  message: 'Target peer not found',
                })
              );
              break;
            }
            const connectedPeer = peers.get(connectedPeerName)!;
            console.log('sending to server');
            connectedPeer.socket.send(
              JSON.stringify({
                type: message.type,
                from: peerName,
                data: message.data,
              })
            );
            break;
          }
          case 'candidate': {
            peerName = message.name;
            // peerType = message.peerType;
            if (!peers.has(peerName)) {
              // Name already taken
              sock.send(
                JSON.stringify({ type: 'error', message: `Name don't exist` })
              );
              sock.close();
              return;
            }
            peers.get(peerName)?.candidates.push(message.data);
            console.log(`Forwarding candidate: ${peerName} (${peerType})`);
            const connectedPeerName = message.target;
            if (!connectedPeerName || !peers.has(connectedPeerName)) {
              sock.send(
                JSON.stringify({
                  type: 'error',
                  message: 'Target peer not found',
                })
              );
              break;
            }
            const connectedPeer = peers.get(connectedPeerName)!;
            console.log('sending to server');
            connectedPeer.socket.send(
              JSON.stringify({
                type: 'candidate',
                from: peerName,
                data: message.data,
              })
            );
            break;
          }
          default: {
            sock.send(
              JSON.stringify({ type: 'error', message: 'Unknown message type' })
            );
            break;
          }
        }

      } catch (e) {
        console.error(e);
      }

    };
    sock.onclose = (ev) => {
      // Handle peer disconnection
      console.log(`Peer disconnected: ${peerName}`);
      const connectedPeerName = peers.get(peerName)?.connectedTo;
      if (connectedPeerName && peers.has(connectedPeerName)) {
        // Notify connected peer
        const connectedPeer = peers.get(connectedPeerName)!;
        connectedPeer.socket.send(
          JSON.stringify({ type: 'peer_disconnected', name: peerName })
        );
        connectedPeer.connectedTo = undefined;
      }
      peers.delete(peerName);
    };
  } catch (err) {
    console.error(`WebSocket error: ${err}`);
    const connectedPeerName = peers.get(peerName)?.connectedTo;
    if (connectedPeerName && peers.has(connectedPeerName)) {
      const connectedPeer = peers.get(connectedPeerName)!;
      connectedPeer.socket.send(
        JSON.stringify({ type: 'peer_disconnected', name: peerName })
      );
      connectedPeer.connectedTo = undefined;
    }
    peers.delete(peerName);
  }
}

Deno.serve({ port: 8002, hostname: '0.0.0.0' }, (req) => {
  // console.log(req);

  if (req.headers.get('upgrade') != 'websocket') {
    // console.log(req, req.headers.get('upgrade'));
    return new Response(null, { status: 501 });
  }
  const { socket, response } = Deno.upgradeWebSocket(req);
  handleWs(socket);
  return response;
});
