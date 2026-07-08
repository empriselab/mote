<script lang="ts">
    import { errors, dismiss } from "./errors.svelte";
</script>

<div class="toast-stack">
    {#each errors as error (error.id)}
        <div class="toast failed">
            <span class="toast-message">{error.message}</span>
            <button
                class="toast-dismiss"
                aria-label="Dismiss error"
                onclick={() => dismiss(error.id)}
                ><span class="press">[✖]</span></button
            >
        </div>
    {/each}
</div>

<style>
    .toast-stack {
        position: fixed;
        bottom: 1ch;
        right: 1ch;
        z-index: 100;
        display: flex;
        flex-direction: column-reverse;
        gap: 1ch;
        max-width: min(50ch, 90vw);
    }

    .toast {
        /* Cancel the theme's `* + *` top margin so stacked toasts align. */
        margin-top: 0;
        display: flex;
        align-items: flex-start;
        gap: 1ch;
        padding: calc(var(--line-height) / 2) 1ch;
        border: var(--border-thickness) solid salmon;
        background: var(--background-color);
    }

    .toast-message {
        flex: 1;
        color: salmon;
        white-space: pre-wrap;
        word-break: break-word;
    }

    .toast-dismiss {
        color: salmon;
        flex: none;
    }
</style>
