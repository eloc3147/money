import { el, RedomComponent } from "redom";
import { Money, UploadSession } from "../money-web/pkg/money_web";
import { Table, ColumnView } from "./components";
import { Page } from "./page";


export class UploadPage implements Page {
    client: Money;

    title: HTMLParagraphElement;
    subtitle: HTMLParagraphElement;
    upload_select: UploadSelect;
    error_label: HTMLDivElement;
    error_box: HTMLElement;

    input_field: HTMLDivElement;
    load_field: HTMLDivElement;

    show_more_wrapper: HTMLFieldSetElement;
    submit_wrapper: HTMLFieldSetElement;
    show_more_button: HTMLButtonElement;
    submit_button: HTMLButtonElement;

    el: ColumnView;

    constructor(client: Money) {
        this.client = client;

        this.title = null;
        this.subtitle = null;
        this.upload_select = null;

        this.input_field = null;
        this.load_field = null;

        this.el = new ColumnView("is-half");
    }

    onmount() {
        this.title = el("p", { class: "title is-1" }, "Add Transactions");
        this.subtitle = el("p", { class: "subtitle is-3" }, "Select a file");
        this.error_label = el("div", { className: "message-body is-hidden" }, "");
        this.error_box = el("article", { className: "message is-danger" }, this.error_label);
        this.upload_select = new UploadSelect(this);

        this.el.set_contents([
            this.title,
            this.subtitle,
            this.error_box,
            this.upload_select
        ]);
    }

    onremount() {
        this.el.set_contents([
            this.title,
            this.subtitle,
            this.error_box,
            this.upload_select
        ]);
    }

    set_error(error_msg: string) {
        if (error_msg !== null) {
            this.error_label.textContent = error_msg;
            this.error_label.className = "message-body";
        } else {
            this.error_label.textContent = "";
            this.error_label.className = "message-body is-hidden";
        }
    }

    load_file(file: File) {
        var reader = new FileReader();
        reader.onloadend = _evt => {
            if (reader == null) {
                console.log("Error: reader is null.");
                return;
            }

            this.draw_preview(reader);
        };

        reader.readAsText(file);
    }

    draw_preview(reader: FileReader) {
        this.set_error(null);
        let session = this.client.load_file(reader);

        this.subtitle.innerText = "Select the types of each column";

        this.el.set_column_args("is-full");
        this.el.set_contents([
            this.title,
            this.subtitle,
            this.error_box,
            new UploadPreview(this, session)
        ])
    }

    draw_submitted() {
        this.set_error(null);
        this.el.set_contents([
            this.title,
            this.error_box,
            new UploadSubmitted()
        ]);
    }
}


class UploadSelect implements RedomComponent {
    el: HTMLDivElement;

    upload_page: UploadPage;
    file_field: HTMLInputElement;
    load_button: HTMLButtonElement;

    constructor(upload_page: UploadPage) {
        this.upload_page = upload_page;

        this.el = el("div", [
            el("div.field", [
                el("label.label", "File upload"),
                el("div.control", this.file_field = el("input", { type: "file", class: "input" }))
            ]),
            el(
                "div.field",
                el("div.control", this.load_button = el("button", { class: "button is-link" }, "Load file"))
            )
        ]);

        this.load_button.onclick = (evt) => {
            evt.preventDefault();

            if (this.file_field.files.length != 1) {
                this.upload_page.set_error("Please select one file to upload.")
            } else {
                this.upload_page.set_error(null);
            }

            let reader = new FileReader();
            reader.onloadend = (_E) => {
                fetch("/api/upload/", { method: "post", body: reader.result }).then((resp) => {
                    console.log("Upload response", resp);
                })
            };

            reader.readAsArrayBuffer(this.file_field.files[0]);
        };
    }
}


class UploadPreview implements RedomComponent {
    session: UploadSession;
    upload_page: UploadPage;
    current_row_count: number;

    el: HTMLDivElement;
    table: Table;
    show_more_button: HTMLButtonElement;
    show_more_wrapper: HTMLFieldSetElement;
    submit_button: HTMLButtonElement;
    submit_wrapper: HTMLFieldSetElement;

    constructor(upload_page: UploadPage, session: UploadSession) {
        this.upload_page = upload_page;
        this.session = session;
        this.current_row_count = 0;

        this.table = new Table();
        this.table.set_headers(this.session.get_headers().map(h => '"' + h + '"'));
        this.table.set_suggestions(
            this.session.get_header_suggestions(),
            (column_index, selection) => this.process_update(column_index, selection)
        );

        this.el = el("div", [
            this.table,
            el("div", { className: "field is-grouped" }, [
                this.show_more_wrapper = el("fieldset",
                    el("div.control",
                        this.show_more_button = el("button", { class: "button" }, "Show More")
                    )
                ),
                this.submit_wrapper = el("fieldset",
                    el("div.control",
                        this.submit_button = el("button", { class: "button is-link" }, "Load file")
                    )
                )
            ]),
        ]);

        this.show_more_button.onclick = evt => {
            evt.preventDefault();
            this.add_rows();
        };

        this.submit_button.onclick = evt => {
            evt.preventDefault();
            if (!this.check_error()) {
                this.session.submit_data();
                this.upload_page.draw_submitted();
            }
        };

        this.add_rows();
    }

    add_rows(): void {
        let total_row_count = this.session.get_row_count();
        let remaining_rows = Math.max(0, total_row_count - this.current_row_count);
        let row_count = Math.min(10, remaining_rows);

        if (row_count > 0) {
            console.log("Getting rows", this.current_row_count, row_count);
            this.table.add_rows(
                this.session.get_row_slice(this.current_row_count, row_count)
            );
            this.current_row_count += row_count;
        }

        if (this.current_row_count == total_row_count) {
            this.show_more_wrapper.setAttribute("disabled", "true");
        }
    }

    process_update(column_index: number, selection: string): void {
        this.session.update_header_selection(column_index, selection);
        this.check_error();
    }

    check_error(): boolean {
        let selection_error = this.session.get_selection_error();
        if (selection_error !== undefined) {
            this.upload_page.set_error(selection_error);
            this.submit_wrapper.setAttribute("disabled", "true");
            return true;
        } else {
            this.upload_page.set_error(null);
            this.submit_wrapper.removeAttribute("disabled");
            return false;
        }
    }
}

class UploadSubmitted implements RedomComponent {
    el: HTMLElement;

    constructor() {
        this.el = el("article", { className: "message is-primary is-large" }, [
            el("div.message-header", "Upload complete"),
            el("div.message-body", "You can now return to the home page")
        ]);
    }
}