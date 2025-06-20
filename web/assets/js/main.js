"use strict";

const rd = await import("https://cdn.jsdelivr.net/npm/redom@4.3.0/+esm");
const plot = await import("/assets/js/plot.js");


class HomePage {
    constructor() {
        this.el = rd.el("div", [
            rd.el("h1.title", "Home Page (TODO)"),
            new plot.Plot()
        ]);
    }
}


class MoneyApp {
    constructor() {
        this.el = rd.el("div", [
            rd.el("nav.navbar", { role: "navigation", "aria-label": "main navigation", "is-primary": "" }, [
                rd.el("div.navbar-brand", rd.el("span.navbar-item", "Money")),
                rd.el("div.navbar-menu", rd.el("div.navbar-start", this.navbar_items))
            ]),
            this.content = rd.el("div", new HomePage(), { class: "container" })
        ]);
    }
}

rd.mount(document.body, new MoneyApp());
