import type { Terminal } from '@xterm/xterm';
// import ReconnectingWebSocket from 'reconnecting-websocket';
import { toast } from 'svelte-sonner';
import { writable, type Writable } from 'svelte/store';
import { v4 as uuidv4 } from 'uuid';

export type ConnectionData = {
  serverUrl: string;
  targetServer: string;
  targetSession: string;
}

export class ConnectionManager {
  socket: WebSocket;
  pc: RTCPeerConnection;
  myName: string;
  status: Writable<string>;
  sendChannel: RTCDataChannel | null = null;

  constructor(
    public server_url: string,
    public targetServer: string,
    public targetSession: string,
    public term: Terminal,
    public remoteVideo: HTMLVideoElement,
    public onConnected = () => { }
  ) {
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
    const socket = new WebSocket(this.server_url);
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

      this.startSession(this.targetServer, this.targetSession, this.term)
    };
    return socket;
  }

  updatePeerConnection(credentials: { username: string; password: string }) {
    // Update the existing PeerConnection iceServers with new TURN credentials
    const newIceServers: RTCIceServer[] = [
      ...this.pc.getConfiguration().iceServers ?? [],
      {
        urls: `turn:amogos.pro:3478`, // Adjust the TURN server URL as necessary
        username: credentials.username,
        credential: credentials.password,
      }
    ];
    this.pc.setConfiguration({
      iceServers: newIceServers,
    });
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

    const sendChannel = this.pc.createDataChannel('web_shell', dataChannelOptions);
    this.sendChannel = sendChannel

    // const enc = new TextDecoder("utf-8");

    const send = (data: object) => {
      const enc = new TextEncoder();
      const msg = JSON.stringify(data);
      const res = enc.encode(msg);
      sendChannel.send(res);
    }

    sendChannel.onclose = () => this.status.set('sendChannel has closed');
    sendChannel.onopen = () => {
      this.onConnected()
      this.status.set('Send Channel has opened')

      setTimeout(() => {
        const rows = term.rows;
        const cols = term.cols;
        send({
          resize: {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
          }
        });
        console.log('resized');
      }, 5000)

    };

    const dec = new TextDecoder();


    sendChannel.onmessage = async (e) => {
      const data = e.data;
      const decoded = dec.decode(data);
      const msg = JSON.parse(decoded);

      if (msg.output) {
        term.write(msg.output);
      }
    };

    // this.pc.ontrack = (event) => {
    //   console.log('track added', event);

    //   const remoteStream = new MediaStream();
    //   event.streams[0].getTracks().forEach(track => {
    //     remoteStream.addTrack(track);
    //   });
    //   this.remoteVideo.srcObject = remoteStream;
    // }

    this.pc.ontrack = (event) => {
      console.log('track added', event);

      const el = document.createElement(event.track.kind) as HTMLVideoElement
      el.srcObject = event.streams[0]
      el.autoplay = true
      el.controls = true

      event.track.onmute = function (event) {
        console.log(event);
        // el.parentNode?.removeChild(el);
      }

      this.remoteVideo.appendChild(el)
    }




    term.onResize(({ cols, rows }) => {
      send({
        resize: {
          rows,
          cols,
          pixel_width: 0,
          pixel_height: 0,
        }
      });
    });

    this.pc.oniceconnectionstatechange = () => this.status.set(this.pc.iceConnectionState);
    this.pc.onconnectionstatechange = () => this.status.set(this.pc.connectionState);
    this.pc.onsignalingstatechange = () => this.status.set(this.pc.signalingState);
    this.pc.onicegatheringstatechange = () => this.status.set(this.pc.signalingState);

    this.pc.onnegotiationneeded = async () => {
      const offer = await this.pc.createOffer({
        // offerToReceiveAudio: true,
        // offerToReceiveVideo: true,
      });
      await this.pc.setLocalDescription(offer);
      this.socket.send(
        JSON.stringify({
          type: 'offer',
          target: targetServer,
          session: targetSession,
          data: JSON.stringify(this.pc.localDescription)
        })
      );
    };

    term.onData((data: string) => {
      send({ input: data });
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
        case 'offer': {
          console.log('amogus');
          const data = JSON.parse(message.data);
          if (data.type === 'offer') {
            await this.pc.setRemoteDescription(new RTCSessionDescription(data));
          }
          console.log('amogus2');

          const answer = await this.pc.createAnswer({
            // offerToReceiveAudio: true,
            // offerToReceiveVideo: true,
          });
          await this.pc.setLocalDescription(answer);
          console.log('amogus3');

          this.socket.send(
            JSON.stringify({
              type: 'answer',
              target: targetServer,
              session: targetSession,
              data: JSON.stringify(answer)
            })
          );
          console.log('amogus5');

          break;
        }
        case 'answer': {
          const data = JSON.parse(message.data);
          if (data.type === 'answer') {
            await this.pc.setRemoteDescription(new RTCSessionDescription(data));
          }
          // else if (data.candidate) {
          //   await this.pc.addIceCandidate(new RTCIceCandidate(data));
          // }
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

  close() {
    if (this.pc) {
      if (this.sendChannel) {
        this.sendChannel.close();
      }
      this.pc.close();
    }

    if (this.socket) {
      this.socket.close();
    }

    this.status.set('Disconnected');
    toast.info('All connections have been closed.');
  }
}
