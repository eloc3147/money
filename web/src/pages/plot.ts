import { el, RedomComponent, setChildren } from "redom";
import { loadExpenses, loadIncome, TransactionsResponse } from "../api";
import { plot } from "../plotter";

const PLOT_WIDTH = 1920;
const PLOT_HEIGHT = 720;

class Plot implements RedomComponent {
    drawn: boolean;

    loadingBar: HTMLElement;
    el: HTMLElement;

    constructor() {
        this.drawn = false;

        this.loadingBar = el("progress.progress.is-small.is-primary", "15%", { max: 100 });
        this.el = el("div", [this.loadingBar]);
    }

    drawPlot(transactions: TransactionsResponse): void {
        setChildren(this.el, [this.loadingBar]);

        const plotElement = plot(
            transactions,
            PLOT_WIDTH,
            PLOT_HEIGHT
        );

        setChildren(this.el, [plotElement]);
    }

    async onmount(): Promise<void> {
        if (this.drawn) {
            return;
        }

        this.drawPlot(await loadExpenses());
        this.drawn = true;
    }
}

export class PlotPage implements RedomComponent {
    el: HTMLElement;

    constructor() {
        const plot = new Plot();
        let expensesButton = el("button.button.is-light", "Expenses");
        let incomeButton = el("button.button.is-light", "Income");

        expensesButton.onclick = async (_evt: MouseEvent) => {
            plot.drawPlot(await loadExpenses());
        };

        incomeButton.onclick = async (_evt: MouseEvent) => {
            plot.drawPlot(await loadIncome());
        };

        this.el = el("div.container.is-fluid", [
            plot,
            el("div.field.is-grouped.is-grouped-right", [
                el("p.control", expensesButton),
                el("p.control", incomeButton)
            ])
        ])
    }
}
