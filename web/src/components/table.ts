import { el, List, list, RedomComponent } from "redom";
import { TransactionsResponse } from "../api";

class HeaderCell implements RedomComponent {
    el: HTMLTableCellElement;

    constructor(data = "") {
        this.el = el("th", data, { scope: "col" });
    }

    update(data: string): void {
        this.el.textContent = data;
    }
}

class DataCell implements RedomComponent {
    el: HTMLTableCellElement;

    constructor() {
        this.el = el("td");
    }

    update(data: string): void {
        this.el.textContent = data;
    }
}

class DataRow implements RedomComponent {
    el: List;

    constructor() {
        this.el = list("tr", DataCell);
    }

    update(data: number[]): void {
        this.el.update(data);
    }
}

export class Table implements RedomComponent {
    headerColRows: List;
    headerRow: List;
    bodyRows: List;
    el: HTMLDivElement;

    constructor() {
        this.headerColRows = list("tbody", DataRow);
        this.bodyRows = list("tbody", DataRow);
        this.headerRow = list("tr", HeaderCell);

        this.el = el("div", [
            el("table.category-header-table.striped", [
                el("thead", el("tr", new HeaderCell("."))),
                this.headerColRows
            ]),
            el("div.overflow-auto", el("table.striped", [
                el("thead", this.headerRow),
                this.bodyRows
            ]))
        ])
    }

    setTransactions(transactions: TransactionsResponse): void {
        this.headerRow.update(transactions.dates.map((d) => `${d.getFullYear()}-${d.getMonth()}`));
        this.headerColRows.update(transactions.categories.toReversed().map((c) => [c]));

        // Transpose amounts
        const rows: number[][] = [];
        for (let i = 0; i < transactions.categories.length; i++) {
            rows.push([]);
        }

        for (let i = 0; i < transactions.amounts.length; i++) {
            const amounts = transactions.amounts[i] as number[];
            for (let j = 0; j < transactions.categories.length; j++) {
                (rows[transactions.categories.length - j - 1] as number[]).push(amounts[j] as number);
            }
        }

        this.bodyRows.update(rows);
    }
}
