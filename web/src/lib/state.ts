import { persisted } from 'svelte-persisted-store'


export type TerminalState = {
  session: string,
}

export type VideoState = {
  display: number,
}

export type ConnectionData = {
  // client only just for keying
  id: string,
  serverUrl: string;
  targetServer: string;

  videos?: VideoState[];
  terminals?: TerminalState[]
}

type State = {
  connections: ConnectionData[]
}

export const state = persisted<State>('websh-state', {
  connections: []
})