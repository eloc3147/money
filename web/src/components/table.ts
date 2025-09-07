import { el, RedomComponent } from "redom";

export class Table implements RedomComponent {
    el: HTMLTableElement;

    constructor() {
        this.el = el("table");
    }
}
