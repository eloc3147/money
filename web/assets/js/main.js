"use strict";

const rd = await import("https://cdn.jsdelivr.net/npm/redom@4.3.0/+esm");


class PageHeader {
    constructor() {
        this.el = rd.el("header.container-fluid", rd.el("nav", [
            rd.el("ul", rd.el("li", rd.el("strong", "Money"))),
            rd.el("ul", [
                rd.el("li", rd.el("a", "Plot")),
                rd.el("li", rd.el("a", "Other")),
                rd.el("li", rd.el("a", "More Other"))
            ]),
        ]));
    }
}

class PageContents {
    constructor() {
        this.plot = null;
        this.selected = false;  // Turn to enum when there are multiple options

        this.el = rd.el("main.container-fluid");
    }

    async onmount() {
        if (!this.selected) {
            await this.select_plot();
        }
    }

    async select_plot() {
        if (this.plot == null) {
            this.plot = await import("/assets/js/plot.js");
        }

        rd.setChildren(this.el, new this.plot.Plot());
    }
}


class Contents {
    constructor() {
        this.header = new PageHeader();
        this.main = new PageContents();
    }
}

const contents = new Contents();

rd.setChildren(document.body, [contents.header, contents.main])
