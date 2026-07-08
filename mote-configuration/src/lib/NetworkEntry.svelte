<script lang="ts">
    import { tick } from "svelte";
    import ShortSpinner from "./ShortSpinner.svelte";
    import { network_connect } from "./link";
    import {
        open_entry,
        open_entry_field,
        close_entry_field,
    } from "./entry.svelte";
    import { connecting, start_connecting } from "./connecting.svelte";

    let { ssid, strength, is_current_connection } = $props();

    const wifi_strength_indicators = ["[••••]", "[••• ]", "[••  ]", "[•   ]"];

    function get_indicator(strength: number) {
        return wifi_strength_indicators[
            Math.max(
                Math.min(
                    Math.floor(strength / 20) - 1,
                    wifi_strength_indicators.length - 1,
                ),
                0,
            )
        ];
    }

    let input_open = $derived(open_entry.key === ssid);
    let is_connecting = $derived(connecting.ssid === ssid);
    let input_value = $state("");
    let show_password = $state(false);
    let input_ref: HTMLInputElement | undefined;

    // Svelte forbids a dynamic `type` attribute alongside `bind:value`, so
    // toggle visibility by flipping the type on the element imperatively.
    $effect(() => {
        if (input_ref) {
            input_ref.type = show_password ? "text" : "password";
        }
    });

    function submit() {
        network_connect(ssid, input_value);
        close_entry_field(ssid);
        start_connecting(ssid);
    }

    function handle_key(event: KeyboardEvent) {
        if (event.repeat) return;

        if (event.key === "Enter") {
            submit();
        }
    }
</script>

<li class:success={is_current_connection}>
    <span class="label">
        {ssid}
    </span>
    <span class="actions" hidden={!is_current_connection}
        >&lt;~~ currently connected</span
    >
    <span class="actions" hidden={is_current_connection}>
        <pre>{get_indicator(strength)}</pre>
        |{#if is_connecting}
            <span class="connect-slot"><ShortSpinner /></span>
        {:else}
            <button
                id={ssid}
                onclick={async () => {
                    if (input_open) {
                        submit();
                    } else {
                        open_entry_field(ssid);
                        await tick();
                        input_ref?.focus();
                    }
                }}><span class="press">[ connect ]</span></button
            >
        {/if}
    </span>
    <ul hidden={!input_open}>
        <li>
            <div class="password-row">
                <input
                    type="password"
                    name="password"
                    placeholder="enter password"
                    autocomplete="off"
                    bind:this={input_ref}
                    bind:value={input_value}
                    onkeydown={handle_key}
                />
                <button onclick={() => (show_password = !show_password)}
                    ><span class="press"
                        >{show_password ? "[ hide ]" : "[ show ]"}</span
                    ></button
                >
            </div>
        </li>
    </ul>
</li>

<style>
    .password-row {
        display: flex;
        align-items: baseline;
        gap: 1ch;
    }

    .password-row input {
        flex: 1;
        width: auto;
    }

    .password-row > * {
        margin-top: 0;
    }

    .connect-slot {
        display: inline-block;
        width: 11ch;
        text-align: center;
        margin-top: 0;
    }
</style>
