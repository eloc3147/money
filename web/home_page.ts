import { el } from "redom";
import { ColumnView } from "./components";
import { Page } from "./page";

export class HomePage implements Page {
    el: ColumnView;

    constructor() {
        this.el = new ColumnView("is-half");
    }

    onmount() {
        this.el.set_contents([el("h1.title", "Home Page (TODO)")]);
    }
}
