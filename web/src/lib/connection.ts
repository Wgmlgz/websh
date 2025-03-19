import type { Terminal } from '@xterm/xterm';
// import ReconnectingWebSocket from 'reconnecting-websocket';
import { toast } from 'svelte-sonner';
import { writable, type Writable } from 'svelte/store';
import { v4 as uuidv4 } from 'uuid';
import type { ControlMsg } from '../../../bindings/ControlMsg'
import type { DataChannelSettingsMsg } from './../../../bindings/DataChannelSettingsMsg';
import type { ControlMsgBody } from '../../../bindings/ControlMsgBody';
import type { StartVideoMsg } from '../../../bindings/StartVideoMsg';
export class ConnectionManager {
  socket: WebSocket;
  pc: RTCPeerConnection;
  myName: string;
  status: Writable<string>;
  sendChannel: RTCDataChannel | null = null;
  controlChannel: RTCDataChannel | null = null;
  public ready = writable(false)

  constructor(
    public server_url: string,
    public targetServer: string,
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

      this.startSession(this.targetServer)
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

  createDataChannel(msg: DataChannelSettingsMsg, dataChannelOptions: RTCDataChannelInit = {
    //ordered: false,
    //maxPacketLifeTime: 10,
    ordered: true
  }) {
    return this.pc.createDataChannel(JSON.stringify(msg), dataChannelOptions);
  }

  async createControl() {

    const controlChannel = this.createDataChannel({ variant: 'control', session_id: null });
    this.controlChannel = controlChannel

    controlChannel.onclose = () => this.status.set('Control Channel has closed');
    controlChannel.onopen = () => {
      this.ready.set(true)
      this.onConnected()
      this.status.set('Control Channel has opened')
    };

    const dec = new TextDecoder();


    controlChannel.onmessage = async (e) => {
      const data = e.data;
      const decoded = dec.decode(data);
      console.log('huh', decoded);
      const msg = JSON.parse(decoded);
      if (msg.output) {
        console.log(msg.output)
      }
    };
  }

  control_id: number = 0

  getControlId() {
    return ++this.control_id;
  }

  async sendControl(data: ControlMsgBody) {
    if (!this.controlChannel) throw new Error('Control channel not opened yet');
    const enc = new TextEncoder();
    const msg = JSON.stringify({
      id: this.getControlId(),
      body: data,
    } satisfies ControlMsg);
    console.log('Sending control msg', msg);

    const res = enc.encode(msg);
    this.controlChannel.send(res);
  }

  async startWebShell(term: Terminal, session_id: string) {
    const sendChannel = this.createDataChannel({ variant: 'web_shell', session_id });
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
    term.onData((data: string) => {
      send({ input: data });
    });

  }

  async startVideo(remoteVideo: HTMLDivElement, StartVideo: StartVideoMsg) {
    this.sendControl({
      StartVideo
    })
    this.pc.ontrack = (event) => {
      console.log('track added', event);

      const el = document.createElement(event.track.kind) as HTMLVideoElement
      el.srcObject = event.streams[0]
      el.autoplay = true
      el.controls = true
      el.muted = true

      event.track.onmute = function (event) {
        console.log(event);
        // el.parentNode?.removeChild(el);
      }

      remoteVideo.appendChild(el)
    }
  }

  async startSession(targetServer: string) {
    if (!targetServer) {
      return toast.error('Target server name must not be empty');
    }

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
          data: JSON.stringify(this.pc.localDescription)
        })
      );
    };


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

          const answer = await this.pc.createAnswer({
            // offerToReceiveAudio: true,
            // offerToReceiveVideo: true,
          });
          await this.pc.setLocalDescription(answer);

          this.socket.send(
            JSON.stringify({
              type: 'answer',
              target: targetServer,
              data: JSON.stringify(answer)
            })
          );
          break;
        }
        case 'answer': {
          const data = JSON.parse(message.data);
          if (data.type === 'answer') {
            await this.pc.setRemoteDescription(new RTCSessionDescription(data));
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
    this.createControl()
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
