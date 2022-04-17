import { el } from "redom";
import { Money, UploadSession } from "../money-web/pkg/money_web";
import { Table, ColumnView } from "./components";
import { Page } from "./page";


export class UploadPage implements Page {
    client: Money;
    session: UploadSession;
    current_row_count: number;
    preview: Table;

    title: HTMLParagraphElement;
    subtitle: HTMLParagraphElement;

    input_field: HTMLDivElement;
    load_field: HTMLDivElement;

    file_field: HTMLInputElement;
    load_button: HTMLButtonElement;
    error_label: HTMLDivElement;
    show_more_wrapper: HTMLFieldSetElement;
    submit_wrapper: HTMLFieldSetElement;
    show_more_button: HTMLButtonElement;
    submit_button: HTMLButtonElement;

    el: ColumnView;

    constructor(client: Money) {
        this.client = client;
        this.session = null;
        this.preview = null;
        this.current_row_count = 0;

        this.title = null;
        this.subtitle = null;
        this.input_field = null;
        this.load_field = null;

        this.show_more_button = null;
        this.show_more_wrapper = null;
        this.submit_button = null;
        this.submit_wrapper = null;
        this.error_label = null;

        this.el = new ColumnView("is-half");
    }

    onmount() {
        this.title = el("p", { class: "title is-1" }, "Add Transactions");
        this.subtitle = el("p", { class: "subtitle is-3" }, "Select a file");
        this.input_field = el("div", { class: "field" }, [
            el("label.label", "File upload"),
            el("div.control", this.file_field = el("input", { type: "file", class: "input" }))
        ]);
        this.load_field = el(
            "div",
            { class: "field" },
            el("div.control", this.load_button = el("button", { class: "button is-link" }, "Load file"))
        );

        this.load_button.onclick = evt => {
            evt.preventDefault();

            var reader = new FileReader();
            reader.onloadend = _evt => {
                if (reader == null) {
                    console.log("Error: reader is null.");
                    return;
                }

                this.session = this.client.load_file(reader);
                this.draw_preview();
            };

            reader.readAsText(this.file_field.files[0]);
        };

        this.el.set_contents([
            this.title,
            this.subtitle,
            this.input_field,
            this.load_field
        ]);
    }

    onremount() {
        this.el.set_contents([
            this.title,
            this.subtitle,
            this.input_field,
            this.load_field
        ]);
    }

    draw_preview() {
        this.preview = new Table();
        this.preview.set_headers(this.session.get_headers().map(h => '"' + h + '"'));
        this.preview.set_suggestions(
            this.session.get_header_suggestions(),
            (column_index, selection) => this.process_update(column_index, selection)
        );

        this.subtitle.innerText = "Select the types of each column";

        this.el.set_column_args("is-full");
        this.el.set_contents([
            this.title,
            this.subtitle,
            el("article", { className: "message is-danger" },
                this.error_label = el("div", { className: "message-body is-hidden" }, "")
            ),
            this.preview,
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
            if (this.session.get_selection_error() === undefined) {
                this.session.submit_data();
                this.draw_submitted();
            }
        };

        this.file_field = null;
        this.load_button = null;

        this.add_rows();
    }

    add_rows() {
        let total_row_count = this.session.get_row_count();
        let remaining_rows = Math.max(0, total_row_count - this.current_row_count);
        let row_count = Math.min(10, remaining_rows);
        console.log("Getting rows", this.current_row_count, this.current_row_count + row_count);
        this.preview.add_rows(
            this.session.get_row_slice(this.current_row_count, this.current_row_count + row_count)
        );
        this.current_row_count += row_count;

        if (this.current_row_count == total_row_count) {
            this.show_more_wrapper.setAttribute("disabled", "true");
        }
    }

    process_update(column_index: number, selection: string) {
        this.session.update_header_selection(column_index, selection);

        let selection_error = this.session.get_selection_error();
        if (selection_error !== undefined) {
            this.error_label.textContent = selection_error;
            this.error_label.className = "message-body";
            this.submit_wrapper.setAttribute("disabled", "true");
        } else {
            this.error_label.textContent = "";
            this.error_label.className = "message-body is-hidden";
            this.submit_wrapper.removeAttribute("disabled");
        }
    }

    draw_submitted() {
        this.error_label.textContent = "Data submitted.";
        this.error_label.className = "message-body";

        this.el.set_contents([
            this.title,
            el("article", { className: "message is-primary is-large" }, [
                el("div.message-header", "Upload complete"),
                el("div.message-body", "You can now return to the home page")
            ])
        ]);
    }
}
