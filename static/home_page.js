/*jshint esversion: 6 */

import { el } from "https://redom.js.org/redom.es.min.js";
import { ColumnView } from "/components.js";

export class HomePage {
    constructor(args) {
        this.el = new ColumnView("is-half",
            el("h1.title", "Home Page (TODO)")
        );
    }
}
