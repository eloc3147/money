"use strict";

const rd = await import("https://cdn.jsdelivr.net/npm/redom@4.3.0/+esm");

const PALATE = [
    // "#c52f21", // red;
    "#d92662", // pink;
    // "#c1208b", // fuchsia;
    "#9236a4", // purple;
    // "#7540bf", // violet;
    "#524ed2", // indigo;
    // "#2060df", // blue;
    "#0172ad", // azure;
    // "#047878", // cyan;
    "#007a50", // jade;
    // "#398712", // green;
    "#a5d601", // lime;
    // "#f2df0d", // yellow;
    "#ffbf00", // amber;
    "#ff9500", // pumpkin;
    // "#d24317", // orange;
    // "#ccc6b4", // sand;
    "#ababab", // grey;
    // "#646b79", // zinc;
    "#525f7a", // slate;
];


export class Plot {
    constructor() {
        this.d3 = null;
        this.drawn = false;

        this.container_width = 1920;
        this.container_height = 720;
        this.margin = { top: 60, right: 160, bottom: 50, left: 50 };
        this.width = this.container_width - this.margin.left - this.margin.right;
        this.height = this.container_height - this.margin.top - this.margin.bottom;

        this.data = null;
        this.stacked_data = null;
        this.max_height = 0;
        this.color_map = null;

        this.idle_timeout = null;
        this.selector = null;
        this.x = null;
        this.x_axis = null;
        this.area_container = null;
        this.area = null;

        this.el = rd.el("div", { "aria-busy": true });
    }

    async onmount() {
        await this.update_plot();
    }

    async update_plot() {
        if (this.drawn) {
            return;
        }

        let [_, loaded_data] = await Promise.all([
            this.import_d3(),
            this.load_data()
        ]);

        if (loaded_data) {
            // Requires both data and d3
            this.process_data();
        }

        rd.setChildren(this.el, await this.draw());
        this.el.removeAttribute("aria-busy");
    }

    async import_d3() {
        if (this.d3 != null) {
            return;
        }

        this.d3 = await import("https://cdn.jsdelivr.net/npm/d3@7.9.0/+esm");
    }

    async load_data() {
        if (this.data != null) {
            return false;
        }

        const resp = await fetch("/api/test_data");
        if (!resp.ok) {
            throw new Error(`Loading data failed with code: ${resp.status}`);
        }

        this.data = await resp.json();
        return true;
    }

    process_data() {
        this.stacked_data = this.d3.stack().keys(Array(this.data.keys.length).keys())(this.data.rows);
        this.max_height = 0;
        for (const row of this.stacked_data) {
            for (const cell of row) {
                this.max_height = Math.max(this.max_height, cell[1]);
            }
        }

        if (this.data.keys.length > PALATE.length) {
            throw new Error(`Fewer color palate options (${PALATE.length}) than data keys (${this.data.keys.length})`);
        }

        this.color_map = new Map();
        for (let i = 0; i < this.data.keys.length; i++) {
            this.color_map.set(i, PALATE[i]);
        }
    }

    // TODO: Style plot
    async draw() {
        const d3 = this.d3;

        // Main Container
        const svg = d3.create("svg")
            .attr("viewBox", `-20, -40, ${this.container_width}, ${this.container_height}`)
            .attr("preserveAspectRatio", "xMidYMid meet");

        // Axis

        // Create scaling functions
        this.x = d3.scaleLinear()
            .domain([0, this.data.rows.length - 1])
            .range([0, this.width]);

        const y = d3.scaleLinear()
            .domain([0, this.max_height])
            .range([this.height, 0]);

        // Append axis ticks
        this.x_axis = svg.append("g")
            .attr("transform", `translate(0, ${this.height})`)
            .call(d3.axisBottom(this.x).ticks(10));

        svg.append("g")
            .call(d3.axisLeft(y).ticks(5));

        // Append axis labels
        svg.append("text")
            .attr("text-anchor", "end")
            .attr("x", this.width)
            .attr("y", this.height + 40)
            .text("TODO: X Value");

        svg.append("text")
            .attr("text-anchor", "end")
            .attr("x", 0)
            .attr("y", -20)
            .text("TODO: Y Value")
            .attr("text-anchor", "start");

        // Chart area

        // Clipping area to allow selecting a subset of data
        const clip = svg.append("defs")
            .append("clipPath")
            .attr("id", "clip")
            .append("rect")
            .attr("width", this.width)
            .attr("height", this.height)
            .attr("x", 0)
            .attr("y", 0);

        // Area container
        this.area_container = svg.append('g')
            .attr("clip-path", "url(#clip)");

        // Area generator
        this.area = d3.area()
            .x((d, i) => this.x(i))
            .y0((d) => y(d[0]))
            .y1((d) => y(d[1]));

        // Add the data to the chart
        this.area_container
            .selectAll("none")
            .data(this.stacked_data)
            .join("path")
            .attr("class", (d) => "areaTrace trace" + d.key)
            .style("fill", ((d) => this.color_map.get(d.key)).bind(this))
            .attr("d", this.area);

        // Selection box
        this.selector = d3.brushX()
            .extent([[0, 0], [this.width, this.height]])
            .on("end", this.select_chart_section.bind(this));

        // Append the selection box
        this.area_container
            .append("g")
            .attr("class", "brush")
            .call(this.selector);

        // Legend

        // Add one square in the legend for each name.
        const item_size = 20
        svg.selectAll("none")
            .data([...Array(this.data.keys.length).keys()].toReversed())
            .join("rect")
            .attr("x", this.width + 20)
            .attr("y", (d, i) => 10 + i * (item_size + 5))
            .attr("width", item_size)
            .attr("height", item_size)
            .style("fill", ((d) => this.color_map.get(d)).bind(this))
            .on("mouseover", this.highlight.bind(this))
            .on("mouseleave", this.unhighlight.bind(this));

        // Add one dot in the legend for each name.
        svg.selectAll("none")
            .data(this.data.keys.reverse())
            .join("text")
            .attr("x", this.width + 20 + item_size * 1.2)
            .attr("y", (d, i) => 10 + i * (item_size + 5) + 17)
            .style("fill", ((d, i) => this.color_map.get(this.data.keys.length - i - 1)).bind(this))
            .text((d) => d)
            .attr("text-anchor", "left")
            .style("font-size", "20px")
            .on("mouseover", this.highlight.bind(this))
            .on("mouseleave", this.unhighlight.bind(this));

        return svg.node();
    }

    highlight(event, d) {
        // Reduce opacity of all groups
        this.d3.selectAll(".areaTrace").style("opacity", .1);

        // Expect the one that is hovered
        this.d3.select(".trace" + d).style("opacity", 1);
    }

    unhighlight(event, d) {
        this.d3.selectAll(".areaTrace").style("opacity", 1);
    }

    reset_timeout(plot) {
        this.idle_timeout = null;
    }

    select_chart_section(event, d) {
        const extent = event.selection;

        // If no selection, back to initial coordinate. Otherwise, update X axis domain
        if (!extent) {
            // This allows to wait a little bit
            if (!this.idle_timeout) {
                this.idle_timeout = setTimeout(this.reset_timeout.bind(this), 350);
                return;
            }

            this.x.domain([0, this.data.rows.length - 1]);
        } else {
            this.x.domain([this.x.invert(extent[0]), this.x.invert(extent[1])]);

            // This remove the grey brush area as soon as the selection has been done
            this.area_container.select(".brush").call(this.selector.move, null);
        }

        // Update axis and area position
        this.x_axis.transition().duration(1000).call(this.d3.axisBottom(this.x).ticks(5))
        this.area_container
            .selectAll("path")
            .transition().duration(1000)
            .attr("d", this.area);
    }
}
