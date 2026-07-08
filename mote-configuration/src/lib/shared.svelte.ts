import type { MoteToHostMessage, State } from './mote_api_types';
import { push_error } from './errors.svelte';
import { stop_connecting } from './connecting.svelte';

interface MoteTelem {
    latest: Partial<State>
}

export let mote_telem: MoteTelem = $state({ latest: {} });


export function handle_telem_recv(telem: MoteToHostMessage) {
    if (telem !== null && typeof telem === 'object' && 'State' in telem) {
        // Capture the prior connection result before merging so we can detect a
        // transition into a failure and alert the user only once per new error,
        // rather than on every telemetry frame that repeats the same failure.
        const previous = mote_telem.latest.current_network_connection;
        Object.assign(mote_telem.latest, telem.State);

        const current = mote_telem.latest.current_network_connection;
        if (current && 'Err' in current) {
            const previous_error =
                previous && 'Err' in previous ? previous.Err : null;
            if (current.Err !== previous_error) {
                // A newly-failed attempt: alert the user and end the spinner.
                push_error('Failed to connect to network:\n' + current.Err);
                stop_connecting();
            }
        } else if (current && 'Ok' in current) {
            const previous_ok = previous && 'Ok' in previous ? previous.Ok : null;
            if (current.Ok !== previous_ok) {
                // A newly-established connection: end the spinner.
                stop_connecting();
            }
        }
    }
}

const long_spinner_characters = ["⢀⠀", "⡀⠀", "⠄⠀", "⢂⠀", "⡂⠀", "⠅⠀", "⢃⠀", "⡃⠀", "⠍⠀", "⢋⠀", "⡋⠀", "⠍⠁", "⢋⠁", "⡋⠁", "⠍⠉", "⠋⠉", "⠋⠉", "⠉⠙", "⠉⠙", "⠉⠩", "⠈⢙", "⠈⡙", "⢈⠩", "⡀⢙", "⠄⡙", "⢂⠩", "⡂⢘", "⠅⡘", "⢃⠨", "⡃⢐", "⠍⡐", "⢋⠠", "⡋⢀", "⠍⡁", "⢋⠁", "⡋⠁", "⠍⠉", "⠋⠉", "⠋⠉", "⠉⠙", "⠉⠙", "⠉⠩", "⠈⢙", "⠈⡙", "⠈⠩", "⠀⢙", "⠀⡙", "⠀⠩", "⠀⢘", "⠀⡘", "⠀⠨", "⠀⢐", "⠀⡐", "⠀⠠", "⠀⢀", "⠀⡀"];
const short_spinner_characters = ["⣷", "⣯", "⣟", "⡿", "⢿", "⣻", "⣽", "⣾"];

export const long_spinner_state = $state({
    character: long_spinner_characters[0],
    count: 0
});

export const short_spinner_state = $state({
    character: short_spinner_characters[0],
    count: 0
});

$effect.root(() => {
    const interval = setInterval(() => {
        long_spinner_state.count = (long_spinner_state.count + 1) % long_spinner_characters.length;
        long_spinner_state.character = long_spinner_characters[long_spinner_state.count];

        short_spinner_state.count = (short_spinner_state.count + 1) % short_spinner_characters.length;
        short_spinner_state.character = short_spinner_characters[short_spinner_state.count];
    }, 100);
    return () => {
        clearInterval(interval);
    };
}
)
