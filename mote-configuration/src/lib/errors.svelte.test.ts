import { describe, it, expect, beforeEach } from 'vitest';
import { errors, push_error, dismiss } from './errors.svelte';

describe('errors store', () => {
    beforeEach(() => {
        // errors is a shared module-level $state array; reset between tests.
        errors.splice(0, errors.length);
    });

    it('push_error appends a message', () => {
        push_error('boom');
        expect(errors).toHaveLength(1);
        expect(errors[0].message).toBe('boom');
    });

    it('assigns unique ids to consecutive errors', () => {
        const a = push_error('first');
        const b = push_error('second');
        expect(a).not.toBe(b);
        expect(errors.map((e) => e.id)).toEqual([a, b]);
    });

    it('dismiss removes the matching error and leaves others', () => {
        const a = push_error('first');
        const b = push_error('second');
        dismiss(a);
        expect(errors).toHaveLength(1);
        expect(errors[0].id).toBe(b);
    });

    it('dismiss is a no-op for an unknown id', () => {
        push_error('first');
        dismiss(9999);
        expect(errors).toHaveLength(1);
    });

    it('errors persist until explicitly dismissed', () => {
        const id = push_error('sticky');
        expect(errors).toHaveLength(1);
        dismiss(id);
        expect(errors).toHaveLength(0);
    });
});
