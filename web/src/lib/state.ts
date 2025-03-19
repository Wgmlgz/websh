import { persisted } from 'svelte-persisted-store'


export type TerminalState = {
  session: string,
}

export type VideoState = {
  id: string,
  display_id: number,
  width: number | null,
  height: number | null,
  refresh_rate: number | null
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