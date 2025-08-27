import { TransactionsResponse, loadExpenses, loadIncome } from "./api";
import { el, setChildren } from "redom";
import { DataPane } from "./data_pane";


enum Page {
    Expenses,
    Income,
}


class PageHeader {
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


class PageContents {
    selected: Page;
    loaded: Page | null;

    dataPane: DataPane;
    el: HTMLElement;

    constructor() {
        this.selected = Page.Expenses;
        this.loaded = null;

        this.dataPane = new DataPane();
        this.el = el("main.container-fluid", this.dataPane);
    }

    async onmount() {
        if (!this.selected) {
            await this.updatePage();
        }
    }

    async selectPage(page: Page) {
        this.selected = page;
        await this.updatePage();
    }

    async updatePage() {
        if (this.loaded === this.selected) {
            return;
        }

        let transactions: TransactionsResponse;
        switch (this.selected) {
            case Page.Expenses:
                transactions = await loadExpenses();
                break;
            case Page.Income:
                transactions = await loadIncome();
                break;
            default:
                return;
        }

        this.loaded = this.selected;
        this.dataPane.setTransactions(transactions);
    }
}


class Contents {
    header: PageHeader;
    main: PageContents;

    constructor() {
        this.header = new PageHeader(this);
        this.main = new PageContents();
    }
}

const contents = new Contents();

setChildren(document.body, [contents.header, contents.main]);
