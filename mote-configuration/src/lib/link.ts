import { Link } from 'mote-ffi';
import type { HostToMoteMessage, MoteToHostMessage } from './mote_api_types';
import { push_error } from './errors.svelte';

// Init WASM, init comms link
let link = new Link();

// webserial constructs (Web Serial API — types provided by the runtime environment)
let port: any;
let inputStream: any;
let outputStream: any;

export async function serial_connect(
    connect: () => void,
    disconnect: () => void,
    telemetry_recv: (data: MoteToHostMessage) => void,
) {
    try {
        // See https://github.com/raspberrypi/usb-pid for vid
        const filter = { usbVendorId: 0x2e8a, usbProductId: 0x0009 };
        port = await navigator.serial.requestPort({ filters: [filter] });

        await port.open({ baudRate: 115200 });

        // Read and write the port as raw bytes.
        outputStream = port.writable.getWriter();

        // Send a zero byte (COBS delimiter) to flush any startup noise in the
        // UART buffer on the MCU side before the first real message is sent.
        await outputStream.write(new Uint8Array([0]));

        connect();

        await readLoop(telemetry_recv);

        disconnect();
    } catch (error) {
        const name = (error as { name: string }).name;
        if (name == 'NetworkError') {
            disconnect();
        }
        if (name == 'NotFoundError') {
            console.log('[serial] port selection cancelled by user');
            return;
        }
        console.error('[serial] error:', error);
        push_error('Serial connection error:\n' + String(error));
    }
}

const DECODE_ERROR_THRESHOLD = 5;

async function readLoop(telemetry_recv: (data: MoteToHostMessage) => void) {
    console.log("[serial] start read loop");
    inputStream = port.readable.getReader();

    // Reset per connection session so a fresh connect starts the count over.
    let decode_error_count = 0;

    while (true) {
        const { value, done } = await inputStream.read();
        if (done) {
            console.log('[serial] Input DONE');
            inputStream.releaseLock();
            break;
        }

        // Parse message
        try {
            let message = Array.from(value as Uint8Array);
            link.handle_receive(message);

            // Drain any messages completed by this packet.
            let data = link.poll_receive() as MoteToHostMessage | null;
            while (data !== null) {
                console.log(data);
                telemetry_recv(data);
                data = link.poll_receive() as MoteToHostMessage | null;
            }
        } catch (error) {
            console.warn('[serial] discarded undecodable frame:', error);
            decode_error_count++;
            // Alert once, the first time we cross the threshold, to avoid
            // spamming a toast for every subsequent bad frame.
            if (decode_error_count === DECODE_ERROR_THRESHOLD + 1) {
                push_error('Failed to decode message:\n' + String(error));
            }
        }
    }
}

async function write() {
    if (!outputStream) {
        console.log("[serial] write called by serial connection is not up.");
        return;
    }

    let data = link.poll_transmit() as number[] | null;
    if (!data) {
        console.log("[serial] poll_transmit called but no data was returned.");
    }
    while (data) {
        await outputStream.write(new Uint8Array(data));
        console.log("[serial] [TX] message sent");
        data = link.poll_transmit() as number[] | null;
    }
}

// UI event handlers
export async function set_uid(uid: string, error_handler: () => void) {
    if (uid.length > 3) {
        try {
            const msg: HostToMoteMessage = { SetUID: { uid } };
            link.send(msg);
            await write();
        } catch (error) {
            push_error('Failed to set UID:\n' + String(error));
        }
    } else {
        error_handler()
    }
}

export async function network_connect(ssid: string, password: string) {
    try {
        const msg: HostToMoteMessage = { SetNetworkConnectionConfig: { ssid, password } };
        link.send(msg);
        await write();
    } catch (error) {
        push_error('Failed to connect to network:\n' + String(error));
    }
}

export async function rescan() {
    try {
        const msg: HostToMoteMessage = "RequestNetworkScan";
        link.send(msg);
        await write();
    } catch (error) {
        push_error('Failed to request network scan:\n' + String(error));
    }
}
