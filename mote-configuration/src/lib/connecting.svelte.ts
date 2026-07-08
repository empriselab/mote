// Tracks the SSID of an in-progress network connection attempt so the UI can
// show a spinner in place of that network's connect button until the attempt
// resolves (the firmware handles one attempt at a time). Cleared by
// handle_telem_recv when telemetry reports a success or failure.

export const connecting = $state<{ ssid: string | null }>({ ssid: null });

export function start_connecting(ssid: string) {
    connecting.ssid = ssid;
}

export function stop_connecting() {
    connecting.ssid = null;
}
