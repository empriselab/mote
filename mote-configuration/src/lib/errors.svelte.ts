// Global error store, rendered by Toast.svelte in the bottom-right corner.

export interface AppError {
    id: number;
    message: string;
}

export const errors = $state<AppError[]>([]);

let next_id = 0;

// Errors stay on screen until the user dismisses them.
export function push_error(message: string): number {
    const id = next_id++;
    errors.push({ id, message });
    return id;
}

export function dismiss(id: number) {
    const index = errors.findIndex((e) => e.id === id);
    if (index !== -1) {
        errors.splice(index, 1);
    }
}

export function clear_errors() {
    errors.splice(0, errors.length);
}
