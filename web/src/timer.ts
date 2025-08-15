export class Timer {
    expired:  boolean;
    constructor() {
        this.expired = true;
    }

    set(duration: number) {
        this.expired = false;
        setTimeout((() => {this.expired = true;}), duration);
    }
}
