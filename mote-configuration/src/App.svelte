<script lang="ts">
    import { handle_telem_recv, mote_telem } from "./lib/shared.svelte";
    import { rescan, serial_connect } from "./lib/link";
    import { clear_errors } from "./lib/errors.svelte";
    import type { ResultOfStringOr_ConnectionError } from "./lib/mote_api_types";

    // The connected SSID, or null when disconnected or the last attempt failed.
    // (Failures are surfaced to the user as a toast by handle_telem_recv.)
    function connected_ssid(
        conn: ResultOfStringOr_ConnectionError | null | undefined,
    ): string | null {
        return conn && "Ok" in conn ? conn.Ok : null;
    }

    import LongSpinner from "./lib/LongSpinner.svelte";

    import Identification from "./lib/Identification.svelte";
    import Networks from "./lib/Networks.svelte";
    import Diagnostics from "./lib/Diagnostics.svelte";
    import Toast from "./lib/Toast.svelte";

    let serial_connection = $state({
        connected: false,
        has_received: false,
        time_since_received: 0,
        last_telem_time: new Date(),
    });

    $effect(() => {
        const interval = setInterval(() => {
            serial_connection.time_since_received =
                new Date().getTime() -
                serial_connection.last_telem_time.getTime();
        }, 100);
        return () => {
            clearInterval(interval);
        };
    });
</script>

<main>
    <div class="section tree">
        <ul>
            <p class="label"><strong>Mote</strong></p>
            <li class="preconnection">
                Serial: <span
                    class={serial_connection.connected ? "success" : "failed"}
                    >{serial_connection.connected
                        ? "connected"
                        : "disconnected"}</span
                >
                {#if !serial_connection.connected}
                    <button
                        class="actions"
                        onclick={() => {
                            serial_connect(
                                () => {
                                    serial_connection.connected = true;
                                    // Clear stale errors from any prior session.
                                    clear_errors();
                                },
                                () => {
                                    serial_connection.connected = false;
                                    clear_errors();
                                },
                                (telem) => {
                                    serial_connection.last_telem_time =
                                        new Date();
                                    serial_connection.has_received = true;
                                    handle_telem_recv(telem);
                                },
                            );
                        }}><span class="press">[ connect ]</span></button
                    >
                {/if}
            </li>
            <li>
                Telemetry: <span
                    class={serial_connection.connected &&
                    serial_connection.time_since_received <= 1000
                        ? "success"
                        : "failed"}
                    >{serial_connection.connected
                        ? (
                              serial_connection.time_since_received / 1000
                          ).toFixed(2) + "s ago"
                        : "never"}</span
                >
            </li>
            {#if serial_connection.has_received}
                <li class:dimmed={!serial_connection.connected}>
                    <p class="label"><strong>Identification</strong></p>
                    <ul>
                        {#if mote_telem.latest?.uid}
                            <Identification
                                uid={mote_telem.latest?.uid}
                                ip={mote_telem.latest?.ip}
                                mac={mote_telem.latest?.mac}
                            />
                        {:else}
                            <li>
                                <LongSpinner />
                            </li>
                        {/if}
                    </ul>
                </li>
                <li class:dimmed={!serial_connection.connected}>
                    <p class="label">
                        <strong>Networks</strong>
                        {#if mote_telem.latest?.uid}
                            <button class="actions" onclick={rescan}>
                                <span class="press">[ refresh ]</span>
                            </button>
                        {/if}
                    </p>
                    <ul>
                        {#if (mote_telem.latest?.available_network_connections?.length ?? 0) > 0}
                            <Networks
                                networks={mote_telem.latest
                                    ?.available_network_connections}
                                current_connection={connected_ssid(
                                    mote_telem.latest
                                        ?.current_network_connection,
                                )}
                            />
                        {:else if mote_telem.latest?.available_network_connections}
                            <li>No networks available</li>
                        {:else}
                            <li><LongSpinner /></li>
                        {/if}
                    </ul>
                </li>
                <li class:dimmed={!serial_connection.connected}>
                    <p class="label"><strong>Diagnostics</strong></p>
                    <ul>
                        {#if mote_telem.latest?.built_in_test}
                            <Diagnostics
                                diagnostics={mote_telem.latest?.built_in_test}
                            />
                        {:else}
                            <li><LongSpinner /></li>
                        {/if}
                    </ul>
                </li>
            {/if}
        </ul>
    </div>
</main>

<Toast />
