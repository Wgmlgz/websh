import * as WebSocket from 'ws';
import * as net from 'net';

// Define the SSH server host and port
const SSH_SERVER_HOST = 'localhost';
const SSH_SERVER_PORT = 22;

// Create a WebSocket server on port 3005
const wss = new WebSocket.Server({ port: 3005 }, () => {
  console.log('WebSocket server listening on port 3005');
});

wss.on('connection', (ws) => {
  console.log('WebSocket client connected');

  // Create a TCP connection to the SSH server
  const tcpSocket = net.createConnection(
    {
      host: SSH_SERVER_HOST,
      port: SSH_SERVER_PORT,
    },
    () => {
      console.log('Connected to SSH server');
    },
  );

  // Pipe data between the WebSocket and the TCP socket
  ws.on('message', (message) => {
    console.log(message.toString());
    tcpSocket.write(message as any);
  });

  tcpSocket.on('data', (data) => {
    ws.send(data);
  });

  // Handle TCP socket errors
  tcpSocket.on('error', (err) => {
    console.error('TCP socket error:', err.message);
    ws.close();
  });

  // Handle WebSocket errors
  ws.on('error', (err) => {
    console.error('WebSocket error:', err.message);
    tcpSocket.end();
  });

  // Close TCP socket when WebSocket closes
  ws.on('close', () => {
    console.log('WebSocket client disconnected');
    tcpSocket.end();
  });

  // Close WebSocket when TCP socket ends
  tcpSocket.on('end', () => {
    console.log('Disconnected from SSH server');
    ws.close();
  });
});
