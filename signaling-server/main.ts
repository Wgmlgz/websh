interface Peer {
  name: string;
  socket: WebSocket;
  type: 'user' | 'server';
  connectedTo?: string;
}

const peers = new Map<string, Peer>();

function handleWs(sock: WebSocket) {
  console.log('WebSocket connection established');

  let peerName = '';
  let peerType: 'user' | 'server' = 'user';

  try {
    sock.onmessage = ev => {
      const data = ev.data;

      const message = JSON.parse(data);
      if (!message) return;

      switch (message.type) {
        case 'register':
          peerName = message.name;
          peerType = message.peerType;
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
          });
          console.log(`Peer registered: ${peerName} (${peerType})`);
          break;

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
        case 'signal': {
          const session = message.session;

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
              type: 'signal',
              from: peerName,
              session,
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
    };
    sock.onclose = ev => {
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

// console.log('Signaling server running on ws://localhost:8080');

// for await (const req of serve(':8080')) {
//   const { conn, r: bufReader, w: bufWriter, headers } = req;
//   acceptWebSocket({
//     conn,
//     bufReader,
//     bufWriter,
//     headers,
//   })
//     .then(handleWs)
//     .catch(async err => {
//       console.error(`Failed to accept websocket: ${err}`);
//       await req.respond({ status: 400 });
//     });
// }

Deno.serve(req => {
  if (req.headers.get('upgrade') != 'websocket') {
    return new Response(null, { status: 501 });
  }
  const { socket, response } = Deno.upgradeWebSocket(req);
  handleWs(socket);
  return response;
});
