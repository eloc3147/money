import { el, List, list, RedomComponent } from "redom";
import { TransactionsResponse } from "../api";

class HeaderCell implements RedomComponent {
    el: HTMLTableCellElement;

    constructor() {
        this.el = el("th", { "scope": "col" });
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
    headerRow: List;
    bodyRows: List;
    el: HTMLTableElement;

    constructor() {
        this.headerRow = list("tr", HeaderCell);
        this.bodyRows = list("tbody", DataRow);
        this.el = el("table", [
            el("thead", this.headerRow),
            this.bodyRows
        ]);
    }

    setTransactions(transactions: TransactionsResponse): void {
        const headers = [""];
        for (const d of transactions.dates) {
            headers.push(`${d.getFullYear()}-${d.getMonth()}`)
        }
        this.headerRow.update(headers);

        const rows: (number | string)[][] = [];
        for (const c of transactions.categories.toReversed()) {
            rows.push([c]);
        }

        // Transpose amounts
        for (let i = 0; i < transactions.amounts.length; i++) {
            const amounts = transactions.amounts[i] as number[];
            for (let j = 0; j < transactions.categories.length; j++) {
                (rows[transactions.categories.length - j - 1] as (number | string)[]).push(amounts[j] as number);
            }
        }

        this.bodyRows.update(rows);
    }

    setData(data: number[][]) {
        this.bodyRows.update(data);
    }
}
