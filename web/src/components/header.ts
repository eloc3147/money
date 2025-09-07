import { el, RedomComponent } from "redom";
import { Contents, Page } from "../money";

export class PageHeader implements RedomComponent {
    expenseButton: HTMLAnchorElement;
    incomeButton: HTMLAnchorElement;
    el: HTMLElement;

    constructor(contents: Contents) {
        this.el = el("header.container-fluid", el("nav", [
            el("ul", el("li", el("strong", "Money"))),
            el("ul", [
                el("li", this.expenseButton = el("a", "Expenses")),
                el("li", this.incomeButton = el("a", "Income"))
            ]),
        ]));

        this.expenseButton.onclick = async (_evt: MouseEvent) => {
            await contents.main.selectPage(Page.Expenses);
        };

        this.incomeButton.onclick = async (_evt: MouseEvent) => {
            await contents.main.selectPage(Page.Income);
        };
    }
}
