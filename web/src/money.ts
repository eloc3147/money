import { RedomComponent, el, setChildren } from "redom";
import { PlotPage } from "./pages/plot";


export enum Page {
    Plot,
}

class PageHeader implements RedomComponent {
    el: HTMLElement;

    constructor(contents: Contents) {
        const logoButton = el("a.navbar-item", el("strong", "Money"));
        const plotButton = el("a.navbar-item", "Plot");

        logoButton.onclick = (_evt: MouseEvent) => {
            contents.main.selectPage(Page.Plot);
        };

        plotButton.onclick = (_evt: MouseEvent) => {
            contents.main.selectPage(Page.Plot);
        };

        this.el = el("header", el("nav.navbar", [
            el("div.navbar-brand", logoButton),
            el("div.navbar-menu", el("div.navbar-end", [
                plotButton,
            ]))
        ], { role: "navigation", "aria-label": "main navigation" }));
    }
}


class PageContents implements RedomComponent {
    selected: Page;
    loaded: Page | null;

    el: HTMLElement;

    constructor() {
        this.selected = Page.Plot;
        this.loaded = null;

        this.el = el("section.section");
    }

    onmount(): void {
        if (!this.selected) {
            this.updatePage();
        }
    }

    selectPage(page: Page): void {
        this.selected = page;
        this.updatePage();
    }

    updatePage(): void {
        if (this.loaded === this.selected) {
            return;
        }

        switch (this.selected) {
            case Page.Plot:
                setChildren(this.el, [new PlotPage()]);
                break;
            default:
                return;
        }

        this.loaded = this.selected;
    }
}

export class Contents {
    header: PageHeader;
    main: PageContents;

    constructor() {
        this.header = new PageHeader(this);
        this.main = new PageContents();
    }
}

const contents = new Contents();

setChildren(document.body, [
    contents.header,
    contents.main,
    el("footer.footer", el("div.content.has-text-centered", el("p", "Made by Kinnon McAlister")))
]);
