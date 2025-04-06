import { el } from "redom";
import { ColumnView, Page, Table } from "./components";
import { get_accounts, add_account } from "./api/account";


export class AccountsPage implements Page {
    el: ColumnView;
    table: Table;
    error_label: HTMLDivElement;
    error_set: boolean;
    add_account_input: HTMLInputElement;
    add_account_submit: HTMLElement;

    constructor() {
        this.table = new Table(["Account Name"]);
        this.error_label = el("div", { className: "message-body is-hidden" }, "");
        this.add_account_input = el("input", { type: "text", placeholder: "Account Name", class: "input" });
        this.add_account_submit = el("button", "Add Account", { class: "button is-link" });

        this.el = new ColumnView("is-half", [
            el("p", { class: "title is-1" }, "Accounts"),
            this.table,
            el("hr"),
            el("article", { className: "message is-danger" }, this.error_label),
            el("form", [
                el("div", [
                    el("div.control", this.add_account_input),
                    el("div.control", this.add_account_submit)
                ], { class: "field is-grouped" })
            ])
        ]);

        this.error_set = false;

        this.add_account_input.oninput = (_evt) => {
            this.set_error(null);
        }

        this.add_account_submit.onclick = async (evt) => {
            evt.preventDefault();
            this.submit();
        };
    }

    async onmount() {
        await this.update_list();
    }

    async onremount() {
        await this.update_list();
    }

    submit() {
        const name = this.add_account_input.value;
        if (name.length < 1) {
            this.set_error("Account name must not be empty");
            return;
        }

        await add_account(name);

        this.add_account_input.value = "";
        await this.update_list();
    }

    set_error(error_msg: string | null) {
        if (error_msg != null) {
            this.error_label.textContent = error_msg;
            this.error_label.className = "message-body";
            this.error_set = true;
        } else {
            if (!this.error_set) {
                return;
            }

            this.error_label.textContent = "";
            this.error_label.className = "message-body is-hidden";
            this.error_set = false;
        }
    }

    async update_list() {
        const accounts = await get_accounts();

        this.table.clear_rows();
        this.table.add_plain_rows(accounts.accounts.map((a) => [a]));
    }
}
