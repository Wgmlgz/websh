import type { Terminal } from '@xterm/xterm';
import ReconnectingWebSocket from 'reconnecting-websocket';
import { toast } from 'svelte-sonner';
import { get, writable, type Writable } from 'svelte/store';
import { v4 as uuidv4 } from 'uuid';

export class ConnectionManager {
  socket: ReconnectingWebSocket;
  pc: RTCPeerConnection;
  myName: string;
  status: Writable<string>;

  constructor(public server_url: Writable<string>) {
    this.socket = this.setupWebSocket();
    this.pc = new RTCPeerConnection({
      iceServers:
        [
          {
            urls: 'stun:stun.l.google.com:19302'
          },
          {
            urls: "stun:stun.relay.metered.ca:80",
          },
          {
            urls: "turn:global.relay.metered.ca:80",
            username: "0b9bb54c33cb97cf80278546",
            credential: "tn2fgenqlFxFvaYc",
          },
          {
            urls: "turn:global.relay.metered.ca:80?transport=tcp",
            username: "0b9bb54c33cb97cf80278546",
            credential: "tn2fgenqlFxFvaYc",
          },
          {
            urls: "turn:global.relay.metered.ca:443",
            username: "0b9bb54c33cb97cf80278546",
            credential: "tn2fgenqlFxFvaYc",
          },
          {
            urls: "turns:global.relay.metered.ca:443?transport=tcp",
            username: "0b9bb54c33cb97cf80278546",
            credential: "tn2fgenqlFxFvaYc",
          },
        ]
    });
    this.myName = uuidv4(); // Replace with user's unique name
    this.status = writable('starting');
  }

  setupWebSocket() {
    const sus = () => {
      return get(this.server_url)
    }
    const socket = new ReconnectingWebSocket(sus);

    socket.onopen = () => {
      // Register with unique name
      socket.send(
        JSON.stringify({
          type: 'register',
          name: this.myName,
          peer_type: 'user'
        })
      );
      this.status.set('Connected to server');
      // onConnected();
    };

    this.server_url.subscribe(() => {
      console.log('sus');
      socket.reconnect();
    })
    return socket;
  }


  updatePeerConnection(credentials: { username: string; password: string }) {
    // Update the existing PeerConnection iceServers with new TURN credentials
    const newIceServers: RTCIceServer[] = [
      ...this.pc.getConfiguration().iceServers ?? [],
      {
        urls: `turn:amogos.pro:3478`, // Adjust the TURN server URL as necessary
        username: credentials.username,
        credential: credentials.password
      }
    ];
    this.pc.setConfiguration({ iceServers: newIceServers });
    this.status.set('TURN credentials updated');
  }


  async startSession(targetServer: string, targetSession: string, term: Terminal) {
    if (!targetServer) {
      return toast.error('Target server name must not be empty');
    }
    // const targetSession = targetSession;
    if (!targetSession) {
      return toast.error('Target session name must not be empty');
    }

    const dataChannelOptions: RTCDataChannelInit = {
      //ordered: false,
      //maxPacketLifeTime: 10,
      ordered: true
    };

    const sendChannel = this.pc.createDataChannel('foo', dataChannelOptions);
    sendChannel.onclose = () => this.status.set('sendChannel has closed');
    sendChannel.onopen = () => this.status.set('sendChannel has opened');

    sendChannel.onmessage = async (e) => {
      const data = e.data;
      term.write(data);
    };

    this.pc.oniceconnectionstatechange = () => this.status.set(this.pc.iceConnectionState);
    this.pc.onconnectionstatechange = () => this.status.set(this.pc.connectionState);
    this.pc.onsignalingstatechange = () => this.status.set(this.pc.signalingState);
    this.pc.onicegatheringstatechange = () => this.status.set(this.pc.signalingState);

    this.pc.onnegotiationneeded = async () => {
      const offer = await this.pc.createOffer();
      await this.pc.setLocalDescription(offer);
      this.socket.send(
        JSON.stringify({
          type: 'signal',
          target: targetServer,
          session: targetSession,
          data: JSON.stringify(this.pc.localDescription)
        })
      );
    };

    term.onData((data: string) => {
      sendChannel.send(data);
    });

    this.socket.onmessage = async (event) => {
      const message = JSON.parse(event.data);
      switch (message.type) {
        case 'turn_credentials':
          // Update RTCPeerConnection with TURN credentials received
          this.updatePeerConnection(JSON.parse(message.data));
          break;
        case 'connection_request':
          // Users don't handle connection requests
          break;
        case 'signal': {
          const data = JSON.parse(message.data);
          if (data.type === 'answer') {
            await this.pc.setRemoteDescription(new RTCSessionDescription(data));
          } else if (data.candidate) {
            await this.pc.addIceCandidate(new RTCIceCandidate(data));
          }
          break;
        }
        case 'candidate': {
          const data = JSON.parse(message.data);
          this.pc.addIceCandidate(data);
          break;
        }
        case 'error':
          console.error('Error:', message.message);
          break;
        default:
          break;
      }
    };

    this.pc.onicecandidate = (event) => {
      if (event.candidate) {
        this.socket.send(
          JSON.stringify({
            type: 'candidate',
            target: targetServer,
            name: this.myName,
            data: JSON.stringify(event.candidate)
          })
        );
      }
    };
    // Send connection request to signaling server
    this.socket.send(
      JSON.stringify({
        type: 'connect',
        target: targetServer
      })
    );
  }
}
