<script lang="ts">
  import Button from '$lib/components/ui/button/button.svelte';
  import Input from '$lib/components/ui/input/input.svelte';
  import { Terminal } from '@xterm/xterm';
  import { onMount } from 'svelte';
  import ReconnectingWebSocket from 'reconnecting-websocket';
  import { toast } from 'svelte-sonner';

  let terminal: HTMLDivElement;
  let targetServer: string;
  let targetSession: string;

  const term = new Terminal();
  const myName = String(Math.random()); // Replace with user's unique name

  let socket: WebSocket;
  let pc = new RTCPeerConnection({
    iceServers: [
      {
        urls: 'stun:stun.l.google.com:19302'
      }
    ]
  });

  onMount(() => {
    term.open(terminal);
    socket = new WebSocket('ws://localhost:8002');
    socket.onopen = () => {
      // Register with unique name
      socket.send(
        JSON.stringify({
          type: 'register',
          name: myName,
          peerType: 'user'
        })
      );

      onConnected();
    };
  });

  const onConnected = () => {};

  const startSession = async () => {
    if (!targetServer) {
      return alert('Target server name must not be empty');
    }
    // const targetSession = targetSession;
    if (!targetSession) {
      return alert('Target session name must not be empty');
    }

    let dataChannelOptions: RTCDataChannelInit = {
      //ordered: false,
      //maxPacketLifeTime: 10,
      ordered: true
    };

    let sendChannel = pc.createDataChannel('foo', dataChannelOptions);
    sendChannel.onclose = () => console.log('sendChannel has closed');
    sendChannel.onopen = () => console.log('sendChannel has opened');

    sendChannel.onmessage = async (e) => {
      const data = e.data;
      console.log(data);
      term.write(data);
    };

    pc.oniceconnectionstatechange = (e) => console.log(pc.iceConnectionState);

    pc.onnegotiationneeded = (e) =>
      pc
        .createOffer()
        .then((d) => pc.setLocalDescription(d))
        .catch(console.log);

    term.onData(function (data) {
      console.log(JSON.stringify(data));
      // socket.send(data);
      sendChannel.send(data);
    });

    socket.onmessage = async (event) => {
      const message = JSON.parse(event.data);
      switch (message.type) {
        case 'connection_request':
          // Users don't handle connection requests
          break;
        case 'signal': {
          const data = JSON.parse(message.data);
          if (data.type === 'answer') {
            await pc.setRemoteDescription(new RTCSessionDescription(data));
          } else if (data.candidate) {
            await pc.addIceCandidate(new RTCIceCandidate(data));
          }
          break;
        }
        case 'candidate': {
          const data = JSON.parse(message.data);
          pc.addIceCandidate(data);
          break;
        }
        case 'error':
          console.error('Error:', message.message);
          break;
        default:
          break;
      }
    };

    pc.onicecandidate = (event) => {
      if (event.candidate) {
        socket.send(
          JSON.stringify({
            type: 'candidate',
            target: targetServer,
            name: myName,
            data: JSON.stringify(event.candidate)
          })
        );
      }
    };
    // Send connection request to signaling server
    socket.send(
      JSON.stringify({
        type: 'connect',
        target: targetServer
      })
    );

    // Create offer and send it via the signaling server
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    socket.send(
      JSON.stringify({
        type: 'signal',
        target: targetServer,
        session: targetSession,
        data: JSON.stringify(pc.localDescription)
      })
    );
  };
</script>

<div>
  <div bind:this={terminal}></div>
  Target Server Name:
  <Input bind:value={targetServer} />
  Target Session Name:
  <Input bind:value={targetSession} />
  <Button on:click={startSession}>Start Session</Button>
  <div id="logs"></div>
</div>
