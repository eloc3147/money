import { el, setChildren } from "redom";
import { TransactionsResponse } from "./api";
import { plot } from "./plotter";

const PLOT_WIDTH = 1920;
const PLOT_HEIGHT = 720;

class Plot {
    drawn: boolean;
    transactions: TransactionsResponse | null;

    el: HTMLDivElement;

    constructor() {
        this.drawn = false;
        this.transactions = null;

        this.el = el("div", { "aria-busy": true });
    }

    setTransactions(transactions: TransactionsResponse) {
        this.transactions = transactions;
        this.drawn = false;
    }

    updatePlot() {
        if (this.drawn) {
            return;
        }

        if (this.transactions ===  null) {
            throw new Error("Transactions must be set before updating plot");
        }

        const plotElement = plot(
            this.transactions,
            PLOT_WIDTH,
            PLOT_HEIGHT
        );

        setChildren(this.el, [plotElement]);
        this.el.removeAttribute("aria-busy");
    }
}

class Table {
    el: HTMLTableElement;

    constructor() {
        this.el = el("table");
    }
}

export class DataPane {
    plot: Plot;
    table: Table;
    el: HTMLElement;

    constructor() {
        this.plot = new Plot();
        this.table = new Table();
        this.el = el("div.dataPane", [
            this.plot,
            this.table
        ]);
    }

    setTransactions(transactions: TransactionsResponse) {
        this.plot.setTransactions(transactions);
        this.plot.updatePlot();
    }
}
