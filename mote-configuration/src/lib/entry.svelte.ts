// Tracks which collapsible entry field is currently open so that only one can
// be visible at a time. Opening one collapses any other. Fields are keyed by a
// unique string (a network SSID, or a fixed key for the identification field).

export const open_entry = $state<{ key: string | null }>({ key: null });

export function open_entry_field(key: string) {
    open_entry.key = key;
}

export function close_entry_field(key: string) {
    if (open_entry.key === key) {
        open_entry.key = null;
    }
}
