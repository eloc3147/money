export class Timer {
    expired: boolean;
    constructor() {
        this.expired = true;
    }

    set(duration: number): void {
        this.expired = false;
        setTimeout((() => { this.expired = true; }), duration);
    }
}
