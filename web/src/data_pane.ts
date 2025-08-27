import { ContainerCoords, plot } from "./plotter";
import { el, setChildren } from "redom";
import { TransactionsResponse } from "./api";

const CONTAINER_COORDS: ContainerCoords = {
    width: 1920,
    height: 720,
    margin: { left: 50, right: 160, top: 60, bottom: 50 },
};

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
            CONTAINER_COORDS
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
