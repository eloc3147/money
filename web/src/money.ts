import { el, setChildren } from "redom";
import { Plot } from "./plot";

class PageHeader {
    el: HTMLElement;

    constructor() {
        this.el = el("header.container-fluid", el("nav", [
            el("ul", el("li", el("strong", "Money"))),
            el("ul", [
                el("li", el("a", "Plot")),
                el("li", el("a", "Other")),
                el("li", el("a", "More Other"))
            ]),
        ]));
    }
}

class PageContents {
    selected: boolean;
    el: HTMLElement;

    constructor() {
        this.selected = false;  // Turn to enum when there are multiple options

        this.el = el("main.container-fluid");
    }

    onmount() {
        if (!this.selected) {
            this.selectPlot();
        }
    }

    selectPlot() {
        setChildren(this.el, [new Plot()]);
    }
}


class Contents {
    header: PageHeader;
    main: PageContents;

    constructor() {
        this.header = new PageHeader();
        this.main = new PageContents();
    }
}

const contents = new Contents();

setChildren(document.body, [contents.header, contents.main]);
