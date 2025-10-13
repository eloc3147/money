import { el, RedomComponent, setChildren } from "redom";
import { loadExpenses, loadIncome, TransactionsByCategoryResponse } from "../../api";
import { plot } from "./plotter";

const PLOT_WIDTH = 1920;
const PLOT_HEIGHT = 720;

class Plot implements RedomComponent {
    el: HTMLElement;

    constructor() {
        this.el = el("div");
    }

    drawPlot(transactions: TransactionsByCategoryResponse): void {
        const plotElement = plot(
            transactions,
            PLOT_WIDTH,
            PLOT_HEIGHT
        );

        setChildren(this.el, [plotElement]);
    }
}

export class PlotPage implements RedomComponent {
    loaded: boolean;
    plot: Plot;
    el: HTMLElement;

    constructor() {
        this.loaded = false;
        this.plot = new Plot();

        let expensesButton = el("button.button.is-light", "Expenses");
        let incomeButton = el("button.button.is-light", "Income");

        expensesButton.onclick = (async (_evt: MouseEvent) => {
            this.plot.drawPlot(await loadExpenses());
        }).bind(this);

        incomeButton.onclick = (async (_evt: MouseEvent) => {
            this.plot.drawPlot(await loadIncome());
        }).bind(this);

        this.el = el("div.container.is-fluid", [
            this.plot,
            el("div.field.is-grouped.is-grouped-right", [
                el("p.control", expensesButton),
                el("p.control", incomeButton)
            ])
        ])
    }

    async onmount(): Promise<void> {
        if (this.loaded) {
            return;
        }

        this.plot.drawPlot(await loadExpenses());
        this.loaded = true;
    }
}
