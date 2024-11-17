import * as WebSocket from 'ws';
import * as net from 'net';

// Define the WebSocket server host and port
const WS_SERVER_HOST = 'localhost';
const WS_SERVER_PORT = 3005;

// Create a TCP server that listens on port 2222
const server = net.createServer((tcpSocket) => {
  console.log('TCP client connected');

  // Create a WebSocket client and connect to the WebSocket server
  const ws = new WebSocket(`ws://${WS_SERVER_HOST}:${WS_SERVER_PORT}`);

  // When the WebSocket connection is open
  ws.on('open', () => {
    console.log('WebSocket connection established');

    // Pipe data between the TCP socket and the WebSocket
    tcpSocket.on('data', (data) => {
      ws.send(data);
    });

    ws.on('message', (message) => {
      tcpSocket.write(message as any);
    });
  });

  // Handle WebSocket errors
  ws.on('error', (err) => {
    console.error('WebSocket error:', err.message);
    tcpSocket.end();
  });

  // Handle TCP socket errors
  tcpSocket.on('error', (err) => {
    console.error('TCP socket error:', err.message);
    ws.close();
  });

  // Close WebSocket when TCP socket ends
  tcpSocket.on('end', () => {
    console.log('TCP client disconnected');
    ws.close();
  });

  // Close TCP socket when WebSocket closes
  ws.on('close', () => {
    console.log('WebSocket connection closed');
    tcpSocket.end();
  });
});

// Start listening on port 2222
server.listen(2222, () => {
  console.log('TCP server listening on port 2222');
});
