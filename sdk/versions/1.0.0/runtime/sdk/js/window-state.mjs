/** Tracks host-owned window geometry. Fullscreen requests resolve only after
 * a WindowState event reports the requested mode; no browser DOM API is used. */
export class WindowStateController {
    constructor(send, { timeoutMs = 5000 } = {}) {
        this.send = send;
        this.timeoutMs = timeoutMs;
        this.state = null;
        this.listeners = new Set();
        this.pending = [];
        this.closed = false;
    }

    onState(listener) {
        if (typeof listener !== 'function') throw new TypeError('Window state listener must be a function');
        if (this.closed) throw new Error('Window is closed');
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }

    setFullscreen(enabled) {
        if (typeof enabled !== 'boolean') return Promise.reject(new TypeError('Fullscreen requires a boolean'));
        if (this.closed) return Promise.reject(new Error('Window is closed'));
        if (this.pending.length >= 16) return Promise.reject(new RangeError('Too many pending window requests'));
        return new Promise((resolve, reject) => {
            this.pending.push({ enabled, resolve, reject, sent: false, timer: null });
            this.pump();
        });
    }

    pump() {
        while (this.pending.length) {
            const request = this.pending[0];
            if (request.sent) return;
            if (this.state?.fullscreen === request.enabled) {
                this.pending.shift();
                request.resolve(this.state);
                continue;
            }
            request.sent = true;
            request.timer = setTimeout(() => this.rejectPending(new Error('Native window fullscreen confirmation timed out')), this.timeoutMs);
            try { this.send('fullscreen', request.enabled); }
            catch (error) { this.rejectPending(error); }
            return;
        }
    }

    observe(state) {
        if (this.closed) return;
        this.state = Object.freeze({ ...state });
        const request = this.pending[0];
        if (request?.sent && request.enabled === state.fullscreen) {
            this.pending.shift();
            clearTimeout(request.timer);
            request.resolve(this.state);
        }
        this.pump();
        for (const listener of [...this.listeners]) listener(this.state);
    }

    rejectPending(error) {
        // A timed-out operation may still reach the host later. Forget the
        // previous mode so a recovery request sends an explicit new command.
        this.state = null;
        for (const request of this.pending.splice(0)) {
            clearTimeout(request.timer);
            request.reject(error);
        }
    }

    close() {
        if (this.closed) return;
        this.closed = true;
        this.rejectPending(new Error('Window closed before fullscreen confirmation'));
        this.listeners.clear();
    }
}

/** Resize requests have their own acknowledgements: ordinary geometry events
 * cannot confirm a request, and clamped host sizes are reported accurately. */
export class WindowSizeController {
    constructor(send, {timeoutMs = 5000} = {}) {
        this.send = send; this.timeoutMs = timeoutMs; this.nextToken = 1n;
        this.pending = []; this.closed = false;
    }
    setContentSize(width, height, {anchor='right'}={}) {
        if (!['right','left','top','bottom'].includes(anchor)) return Promise.reject(TypeError('Invalid content resize anchor'));
        if (this.closed) return Promise.reject(Error('Window is closed'));
        if (![width, height].every(value => Number.isInteger(value) && value >= 96 && value <= 4096)) return Promise.reject(RangeError('Content dimensions must be whole pixels from 96 to 4096'));
        if (this.pending.length >= 16) return Promise.reject(RangeError('Too many pending window size requests'));
        return new Promise((resolve, reject) => {
            this.pending.push({operation:'content-size', token:this.nextToken++, width, height, anchor, resolve, reject});
            this.pump();
        });
    }
    pump() {
        const request = this.pending[0];
        if (!request || request.timer) return;
        request.timer = setTimeout(() => this.failAll(Error('Native window size confirmation timed out')), this.timeoutMs);
        try { this.send({operation:request.operation, token:request.token, width:request.width, height:request.height, anchor:request.anchor}); }
        catch (error) { this.failAll(error); }
    }
    receive(result) {
        const request = this.pending[0];
        if (!request || result.token !== request.token) return;
        this.pending.shift(); clearTimeout(request.timer);
        if (result.error) request.reject(Error(result.error));
        else request.resolve(Object.freeze({width:result.width, height:result.height}));
        this.pump();
    }
    failAll(error) {
        for (const request of this.pending.splice(0)) { clearTimeout(request.timer); request.reject(error); }
    }
    close() { this.closed = true; this.failAll(Error('Window closed before size confirmation')); }
}
