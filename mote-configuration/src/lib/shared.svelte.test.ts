import { describe, it, expect, beforeEach } from 'vitest';
import { handle_telem_recv, mote_telem } from './shared.svelte';
import { errors } from './errors.svelte';
import { connecting, start_connecting } from './connecting.svelte';
import type { MoteToHostMessage } from './mote_api_types';

describe('handle_telem_recv', () => {
    beforeEach(() => {
        // mote_telem.latest, errors and connecting are shared module state; reset.
        for (const key of Object.keys(mote_telem.latest)) {
            delete (mote_telem.latest as Record<string, unknown>)[key];
        }
        errors.splice(0, errors.length);
        connecting.ssid = null;
    });

    it('merges a State message into mote_telem.latest', () => {
        const msg = { State: { uid: 'mote-abc' } } as unknown as MoteToHostMessage;
        handle_telem_recv(msg);
        expect(mote_telem.latest.uid).toBe('mote-abc');
    });

    it('merges subsequent State messages without dropping prior fields', () => {
        handle_telem_recv({ State: { uid: 'mote-abc' } } as unknown as MoteToHostMessage);
        handle_telem_recv({ State: { ip: '10.0.0.5' } } as unknown as MoteToHostMessage);
        expect(mote_telem.latest.uid).toBe('mote-abc');
        expect(mote_telem.latest.ip).toBe('10.0.0.5');
    });

    it('ignores non-State messages', () => {
        handle_telem_recv('Pong' as unknown as MoteToHostMessage);
        expect(Object.keys(mote_telem.latest)).toHaveLength(0);
    });

    it('ignores a null message', () => {
        handle_telem_recv(null as unknown as MoteToHostMessage);
        expect(Object.keys(mote_telem.latest)).toHaveLength(0);
    });

    it('pushes an error when a connection attempt fails', () => {
        handle_telem_recv({
            State: { current_network_connection: { Err: { Other: "timed out" } } },
        } as unknown as MoteToHostMessage);
        expect(errors).toHaveLength(1);
        expect(errors[0].message).toContain("timed out");
    });

    it('does not repeat the error on subsequent identical failures', () => {
        const msg = {
            State: { current_network_connection: { Err: { Other: "timed out" } } },
        } as unknown as MoteToHostMessage;
        handle_telem_recv(msg);
        handle_telem_recv(msg);
        expect(errors).toHaveLength(1);
    });

    it('does not push an error for a successful connection', () => {
        handle_telem_recv({
            State: { current_network_connection: { Ok: "MyWifi" } },
        } as unknown as MoteToHostMessage);
        expect(errors).toHaveLength(0);
    });

    it('stops the connecting spinner once a connection succeeds', () => {
        start_connecting("MyWifi");
        handle_telem_recv({
            State: { current_network_connection: { Ok: "MyWifi" } },
        } as unknown as MoteToHostMessage);
        expect(connecting.ssid).toBeNull();
    });

    it('stops the connecting spinner once a connection fails', () => {
        start_connecting("MyWifi");
        handle_telem_recv({
            State: { current_network_connection: { Err: { Other: "timed out" } } },
        } as unknown as MoteToHostMessage);
        expect(connecting.ssid).toBeNull();
    });

    it('keeps the spinner while the attempt is still in progress', () => {
        start_connecting("MyWifi");
        handle_telem_recv({
            State: { current_network_connection: null },
        } as unknown as MoteToHostMessage);
        expect(connecting.ssid).toBe("MyWifi");
    });
});
