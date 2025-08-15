import * as d3 from "d3";
import { el, setChildren } from "redom";
import { Coordinate, StackData, StackRow, stack } from "./stack_area";
import { TransactionsResponse, loadTransactions } from "./api";
import { Timer } from "./timer";

const PALATE = [
    // "#c52f21", // Red;
    "#d92662", // Pink;
    // "#c1208b", // Fuchsia;
    "#9236a4", // Purple;
    // "#7540bf", // Violet;
    "#524ed2", // Indigo;
    // "#2060df", // Blue;
    "#0172ad", // Azure;
    // "#047878", // Cyan;
    "#007a50", // Jade;
    // "#398712", // Green;
    "#a5d601", // Lime;
    // "#f2df0d", // Yellow;
    "#ffbf00", // Amber;
    "#ff9500", // Pumpkin;
    // "#d24317", // Orange;
    // "#ccc6b4", // Sand;
    "#ababab", // Grey;
    // "#646b79", // Zinc;
    "#525f7a", // Slate;
];
const HIGHLIGHT_OPACITY = 0.1;
const HIGHLIGHT_DEBOUNCE_DELAY = 350;

type Svg = d3.Selection<SVGSVGElement, any, any, any>;
type GElement = d3.Selection<SVGGElement, any, any, any>;
type XScale = d3.ScaleTime<number, number>;
type YScale = d3.ScaleLinear<number, number>;

interface BoxCoords {
    left: number;
    right: number;
    top: number;
    bottom: number;
}

interface ContainerCoords {
    width: number;
    height: number;
    margin: BoxCoords;
}

interface SelectorContainer {
    selector: d3.BrushBehavior<any> | null;
}

function highlightHandler(_event: MouseEvent, data: string) {
    // Reduce opacity of all groups
    d3.selectAll(".areaTrace").style("opacity", HIGHLIGHT_OPACITY);

    // Expect the one that is hovered
    d3.select(`.trace${data}`).style("opacity", 1);
}

function unhighlightHandler(_event: MouseEvent) {
    d3.selectAll(".areaTrace").style("opacity", 1);
}

function buildSelectionHandler(
    dates: Date[],
    xScale: XScale,
    xAxis: GElement,
    areaContainer: GElement,
    area: d3.Area<Coordinate>,
    selector: SelectorContainer,
    timer: Timer,
) {
    return (event: d3.D3BrushEvent<number>) => {
        if (selector.selector === null) {
            throw new Error("Selector not specified for selection handler");
        }

        // If no selection, back to initial coordinate. Otherwise, update X axis domain
        if (event.selection) {
            xScale.domain([xScale.invert(event.selection[0] as number), xScale.invert(event.selection[1] as number)]);

            // This remove the grey brush area as soon as the selection has been done
            areaContainer.select(".brush").call(
                (group: d3.Selection<d3.BaseType, undefined, null, undefined>) => {
                    if (selector.selector === null) {
                        throw new Error("Selector not specified for selection handler");
                    }

                    selector.selector.move(group as unknown as GElement, null);
                }
            );
        } else {
            // This allows to wait a little bit
            if (timer.expired) {
                timer.set(HIGHLIGHT_DEBOUNCE_DELAY);
                return;
            }

            xScale.domain([dates[0] as Date, dates[dates.length - 1] as Date]);
        }

        // Update axis and area position
        xAxis.transition().duration(1000).call(d3.axisBottom(xScale).ticks(5))
        areaContainer
            .selectAll("path")
            .transition().duration(1000)
            .attr("d", (data: any) => area(data));
    };
}

// TODO: Style plot
class Plotter {
    svg: Svg;
    xScale: XScale;
    yScale: YScale;
    width: number;
    height: number;

    constructor(
        xScale: XScale,
        yScale: YScale,
        width: number,
        height: number,
        containerWidth: number,
        containerHeight: number
    ) {
        this.xScale = xScale;
        this.yScale = yScale;
        this.width = width;
        this.height = height;

        this.svg = d3.create("svg")
            .attr("viewBox", `-20, -40, ${containerWidth}, ${containerHeight}`)
            .attr("preserveAspectRatio", "xMidYMid meet");
    }

    drawAxis(): GElement {
        // Append axis ticks
        const xAxis = this.svg.append("g")
            .attr("transform", `translate(0, ${this.height})`)
            .call(d3.axisBottom(this.xScale).ticks(10));

        this.svg.append("g")
            .call(d3.axisLeft(this.yScale).ticks(5));

        // Append axis labels
        this.svg.append("text")
            .attr("text-anchor", "end")
            .attr("x", this.width)
            .attr("y", this.height + 40)
            .text("TODO: X Value");

        this.svg.append("text")
            .attr("text-anchor", "end")
            .attr("x", 0)
            .attr("y", -20)
            .text("TODO: Y Value")
            .attr("text-anchor", "start");

        return xAxis;
    }

    drawClipping() {
        // Clipping area to allow selecting a subset of data
        this.svg.append("defs")
            .append("clipPath")
            .attr("id", "clip")
            .append("rect")
            .attr("width", this.width)
            .attr("height", this.height)
            .attr("x", 0)
            .attr("y", 0);
    }

    drawArea(
        dates: Date[],
        stackData: StackData,
        colorMap: Map<number, string>
    ): [GElement, d3.Area<Coordinate>] {
        // Area container
        const areaContainer = this.svg.append('g')
            .attr("clip-path", "url(#clip)");

        // Area generator
        const area = d3.area()
            .x(((_row: Coordinate, idx: number) => this.xScale(dates[idx] as Date)))
            .y0((coord: Coordinate) => this.yScale(coord[0]))
            .y1((coord: Coordinate) => this.yScale(coord[1]));

        // Add the data to the chart
        areaContainer
            .selectAll("none")
            .data(stackData)
            .join("path")
            .attr("class", (row: StackRow) => `areaTrace trace${row.key}`)
            .style("fill", (row: StackRow) => colorMap.get(row.index) as string)
            .attr("d", area);

        return [areaContainer, area];
    }

    drawSelector(
        dates: Date[],
        xAxis: GElement,
        areaContainer: GElement,
        area: d3.Area<Coordinate>,
    ) {
        const timer = new Timer();
        const selectorContainer: SelectorContainer = { selector: null };
        const selector = d3.brushX()
            .extent([[0, 0], [this.width, this.height]])
            .on("end", buildSelectionHandler(
                dates,
                this.xScale,
                xAxis,
                areaContainer,
                area,
                selectorContainer,
                timer
            ));

        selectorContainer.selector = selector;

        areaContainer
            .append("g")
            .attr("class", "brush")
            .call(selector);
    }

    drawLegend(categories: string[], colorMap: Map<number, string>) {
        // Add one square in the legend for each name.
        const categoriesRev = categories.reverse(), itemSize = 20;
        this.svg.selectAll("none")
            .data(categoriesRev)
            .join("rect")
            .attr("x", this.width + 20)
            .attr("y", (_category: string, idx: number) => 10 + idx * (itemSize + 5))
            .attr("width", itemSize)
            .attr("height", itemSize)
            .style(
                "fill",
                (_category: string, idx: number) => colorMap.get(categoriesRev.length - idx - 1) as string
            )
            .on("mouseover", highlightHandler)
            .on("mouseleave", unhighlightHandler);

        // Add one dot in the legend for each name.
        this.svg.selectAll("none")
            .data(categoriesRev)
            .join("text")
            .attr("x", this.width + 20 + itemSize * 1.2)
            .attr("y", (_category: string, idx: number) => 10 + idx * (itemSize + 5) + 17)
            .style(
                "fill",
                (_category: string, idx: number) => colorMap.get(categoriesRev.length - idx - 1) as string
            )
            .text((category: string) => category)
            .attr("text-anchor", "left")
            .style("font-size", "20px")
            .on("mouseover", highlightHandler)
            .on("mouseleave", unhighlightHandler);
    }

    build(): SVGSVGElement {
        return this.svg.node() as SVGSVGElement;
    }
}

export class Plot {
    drawn: boolean;

    containerCoords: ContainerCoords;
    width: number;
    height: number;

    transactions: TransactionsResponse | null;
    stackedData: StackData | null;
    maxHeight: number;
    colorMap: Map<number, string>;

    el: HTMLDivElement;

    constructor() {
        this.drawn = false;

        this.containerCoords = {
            width: 1920,
            height: 720,
            margin: { left: 50, right: 160, top: 60, bottom: 50 },
        };
        this.width = this.containerCoords.width - this.containerCoords.margin.left - this.containerCoords.margin.right;
        this.height = this.containerCoords.height - this.containerCoords.margin.top - this.containerCoords.margin.bottom;

        this.transactions = null;
        this.stackedData = null;
        this.maxHeight = 0;
        this.colorMap = new Map();

        this.el = el("div", { "aria-busy": true });
    }

    async onmount() {
        await this.updatePlot();
    }

    async updatePlot() {
        if (this.drawn) {
            return;
        }

        this.transactions = await loadTransactions();
        this.stackedData = stack(this.transactions.categories, this.transactions.amounts);
        this.maxHeight = Math.max(...this.stackedData.map(row => Math.max(...row.map(coord => coord[1]))))

        const categoryCount = this.transactions.categories.length;
        if (categoryCount > PALATE.length) {
            console.warn(`Fewer color palate options (${PALATE.length}) than data categories (${categoryCount})`);
            // Throw new Error(`Fewer color palate options (${PALATE.length}) than data categories (${category_count})`);
        }

        for (let idx = 0; idx < categoryCount; idx += 1) {
            this.colorMap.set(idx, PALATE[idx % PALATE.length] as string);
        }

        setChildren(this.el, [this.draw()]);
        this.el.removeAttribute("aria-busy");
    }

    draw(): SVGSVGElement {
        if (this.transactions === null || this.stackedData === null) {
            throw new Error("Data must be loaded before drawing");
        }

        const xScale = d3.scaleTime()
            .domain([this.transactions.dates[0] as Date, this.transactions.dates[this.transactions.dates.length - 1] as Date])
            .range([0, this.width]);

        const yScale = d3.scaleLinear()
            .domain([0, this.maxHeight])
            .range([this.height, 0]);

        const plotter = new Plotter(
            xScale,
            yScale,
            this.width,
            this.height,
            this.containerCoords.width,
            this.containerCoords.height,
        );

        const xAxis = plotter.drawAxis();

        plotter.drawClipping();

        const [areaContainer, area] = plotter.drawArea(this.transactions.dates, this.stackedData, this.colorMap);

        plotter.drawSelector(this.transactions.dates, xAxis, areaContainer, area);

        plotter.drawLegend(this.transactions.categories, this.colorMap);

        return plotter.build();
    }
}
