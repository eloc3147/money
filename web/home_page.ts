import { el } from "redom";
import { ColumnView, Page } from "./components";

export class HomePage implements Page {
    el: ColumnView;

    constructor() {
        this.el = new ColumnView("is-half");
    }

    onmount() {
        this.el.set_contents([el("h1.title", "Home Page (TODO)")]);
    }
}
