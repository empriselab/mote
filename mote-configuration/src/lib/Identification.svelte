<script lang="ts">
    import { tick } from "svelte";
    import ShortSpinner from "./ShortSpinner.svelte";

    import { set_uid } from "./link";
    import { push_error } from "./errors.svelte";
    import {
        open_entry,
        open_entry_field,
        close_entry_field,
    } from "./entry.svelte";

    let { uid, ip, mac } = $props();

    const ENTRY_KEY = "identification";

    let input_open = $derived(open_entry.key === ENTRY_KEY);
    let input_value = $state("");
    let input_ref: HTMLElement;

    function submit() {
        set_uid(input_value, () => {
            push_error("UID must be longer than 3 characters");
        });
        close_entry_field(ENTRY_KEY);
    }

    function handle_key(event: KeyboardEvent) {
        if (event.repeat) return;

        if (event.key === "Enter") {
            submit();
        }
    }
</script>

<li>
    Unique ID: {uid}
    <span class="actions">
        <button
            onclick={async () => {
                if (input_open) {
                    set_uid(input_value, () => {
                        push_error("UID must be longer than 3 characters");
                    });
                    close_entry_field(ENTRY_KEY);
                } else {
                    open_entry_field(ENTRY_KEY);
                    await tick();
                    input_ref.focus();
                }
            }}><span class="press">[ update ]</span></button
        ></span
    >
    <ul hidden={!input_open}>
        <li>
            <input
                type="text"
                id="uid"
                name="uid"
                placeholder="enter new UID"
                autocomplete="off"
                bind:this={input_ref}
                bind:value={input_value}
                onkeydown={handle_key}
            />
        </li>
    </ul>
</li>
<li>
    IP:
    {#if ip}
        {ip}
    {:else}<ShortSpinner />{/if}
</li>
<li>
    MAC:
    {#if mac}
        {mac}
    {:else}<ShortSpinner />{/if}
</li>
