import { RedomComponent, el, setChildren } from "redom";
import { TransactionsResponse, loadExpenses, loadIncome } from "./api";
import { DataPane } from "./components/data_pane";
import { PageHeader } from "./components/header"


export enum Page {
    Expenses,
    Income,
}

class PageContents implements RedomComponent {
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

    async onmount(): Promise<void> {
        if (!this.selected) {
            await this.updatePage();
        }
    }

    async selectPage(page: Page): Promise<void> {
        this.selected = page;
        await this.updatePage();
    }

    async updatePage(): Promise<void> {
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

export class Contents {
    header: PageHeader;
    main: PageContents;

    constructor() {
        this.header = new PageHeader(this);
        this.main = new PageContents();
    }
}

const contents = new Contents();

setChildren(document.body, [contents.header, contents.main]);
