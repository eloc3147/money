/*jshint esversion: 6 */

import { el } from "https://redom.js.org/redom.es.min.js";

export class HomePage {
    constructor(args) {
        this.el = el("div.container", el("h1.title", "Home Page (TODO)"));
    }
}
