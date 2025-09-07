import { RedomComponent, el, setChildren } from "redom";
import { TransactionsResponse } from "../api";
import { plot } from "../plotter";
import { Table } from "./table";

const PLOT_WIDTH = 1920;
const PLOT_HEIGHT = 720;

class Plot implements RedomComponent {
    drawn: boolean;
    transactions: TransactionsResponse | null;

    el: HTMLDivElement;

    constructor() {
        this.drawn = false;
        this.transactions = null;

        this.el = el("div", { "aria-busy": true });
    }

    setTransactions(transactions: TransactionsResponse): void {
        this.transactions = transactions;
        this.drawn = false;
    }

    updatePlot(): void {
        if (this.drawn) {
            return;
        }

        if (this.transactions === null) {
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

export class DataPane implements RedomComponent {
    plot: Plot;
    table: Table;
    el: HTMLElement;

    constructor() {
        this.plot = new Plot();
        this.table = new Table();
        this.el = el("div", [
            this.plot,
            this.table
        ]);
    }

    setTransactions(transactions: TransactionsResponse): void {
        this.plot.setTransactions(transactions);
        this.plot.updatePlot();

        this.table.setTransactions(transactions);
    }
}
