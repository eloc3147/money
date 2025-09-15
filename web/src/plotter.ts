import * as d3 from "d3";
import { Coordinate, StackData, StackRow, stack } from "./stack_area";
import { Timer } from "./timer";
import { TransactionsByCategoryResponse } from "./api";

// Sources from: https://python-graph-gallery.com/color-palette-finder/?palette=signac
const PALATE = [
    "#FBE183FF",
    "#F4C40FFF",
    "#FE9B00FF",
    "#D8443CFF",
    "#9B3441FF",
    "#DE597CFF",
    "#E87B89FF",
    "#E6A2A6FF",
    "#AA7AA1FF",
    "#9F5691FF",
    "#633372FF",
    "#1F6E9CFF",
    "#2B9B81FF",
    "#92C051FF"
]

const HIGHLIGHT_OPACITY = 0.1;
const HIGHLIGHT_DEBOUNCE_DELAY = 350;

const X_TICKS = 10;
const Y_TICKS = 10;

const Y_AXIS_WIDTH = 50;
const X_AXIS_HEIGHT = 40;

const LEGEND_WIDTH = 250;
const LEGEND_TICK_SIZE = 20;
const LEGEND_Y_OFFSET = 10;
const LEGEND_TEXT_Y_OFFSET = 17;
const LEGEND_GAP = 5;

type AnySelection<T extends d3.BaseType> = d3.Selection<T, any, any, any>;

type SvgElement = AnySelection<SVGSVGElement>;
type GElement = AnySelection<SVGGElement>;
type XScale = d3.ScaleTime<number, number>;
type YScale = d3.ScaleLinear<number, number>;

type ColorMap = Map<number, string>;

interface SelectorContainer {
    selector: d3.BrushBehavior<any> | null;
}

function highlightHandler(_event: MouseEvent, data: string): void {
    // Reduce opacity of all groups
    d3.selectAll(".area-trace").style("opacity", HIGHLIGHT_OPACITY);

    // Expect the one that is hovered
    d3.select(`.trace-${data.replace(" ", "-")}`).style("opacity", 1);
}

function unhighlightHandler(_event: MouseEvent): void {
    d3.selectAll(".area-trace").style("opacity", 1);
}

function buildSelectionHandler(
    dates: Date[],
    xScale: XScale,
    xAxis: GElement,
    areaContainer: GElement,
    area: d3.Area<Coordinate>,
    selector: SelectorContainer,
    timer: Timer,
): (event: d3.D3BrushEvent<number>) => void {
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
        xAxis.transition().duration(1000).call(d3.axisBottom(xScale).ticks(X_TICKS))
        areaContainer
            .selectAll("path")
            .transition().duration(1000)
            .attr("d", (data: any) => area(data));
    };
}

function buildColorMap(length: number): ColorMap {
    if (length > PALATE.length) {
        console.warn(`Fewer color palate options (${PALATE.length}) than requested (${length})`);
    }

    const colorMap = new Map();
    for (let idx = 0; idx < length; idx += 1) {
        colorMap.set(idx, PALATE[idx % PALATE.length] as string);
    }

    return colorMap;
}

function drawPlotArea(
    svg: SvgElement,
    dates: Date[],
    stackedData: StackData,
    colorMap: ColorMap,
    xScale: XScale,
    yScale: YScale,
    xAxis: GElement,
    width: number,
    height: number
): void {
    // Clipping area to allow selecting a subset of data
    svg.append("defs")
        .append("clipPath")
        .attr("id", "plot-clip")
        .append("rect")
        .attr("width", width)
        .attr("height", height)
        .attr("x", 0)
        .attr("y", 0);

    // Area container
    const areaContainer = svg.append('g')
        .attr("clip-path", "url(#plot-clip)");

    // Area generator
    const area = d3.area()
        .x(((_row: Coordinate, idx: number) => xScale(dates[idx] as Date)))
        .y0((coord: Coordinate) => yScale(coord[0]))
        .y1((coord: Coordinate) => yScale(coord[1]));

    // Add the data to the chart
    areaContainer
        .selectAll("none")
        .data(stackedData)
        .join("path")
        .attr("class", (row: StackRow) => `area-trace trace-${row.key.replace(" ", "-")}`)
        .style("fill", (row: StackRow) => colorMap.get(row.index) as string)
        .attr("d", area);

    const timer = new Timer();
    const selectorContainer: SelectorContainer = { selector: null };
    const selector = d3.brushX()
        .extent([[0, 0], [width, height]])
        .on("end", buildSelectionHandler(
            dates,
            xScale,
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
        .call(selector as d3.BrushBehavior<any>);
}

function drawLegend(
    svg: SvgElement,
    categories: string[],
    colorMap: ColorMap,
    startX: number,
): void {
    const reversed = categories.toReversed();

    // Add one square in the legend for each name
    svg.selectAll("none")
        .data(reversed)
        .join("rect")
        .attr("x", startX + LEGEND_TICK_SIZE)
        .attr("y", (_category: string, idx: number) => LEGEND_Y_OFFSET + idx * (LEGEND_TICK_SIZE + LEGEND_GAP))
        .attr("width", LEGEND_TICK_SIZE)
        .attr("height", LEGEND_TICK_SIZE)
        .style(
            "fill",
            (_category: string, idx: number) => colorMap.get(categories.length - idx - 1) as string
        )
        .on("mouseover", highlightHandler)
        .on("mouseleave", unhighlightHandler);

    // Add one dot in the legend for each name.
    svg.selectAll("none")
        .data(reversed)
        .join("text")
        .attr("x", startX + LEGEND_TICK_SIZE + LEGEND_TICK_SIZE + LEGEND_GAP)
        .attr("y", (_category: string, idx: number) => LEGEND_Y_OFFSET + LEGEND_TEXT_Y_OFFSET + idx * (LEGEND_TICK_SIZE + LEGEND_GAP))
        .style(
            "fill",
            (_category: string, idx: number) => colorMap.get(categories.length - idx - 1) as string
        )
        .text((category: string) => category)
        .attr("text-anchor", "left")
        .style("font-size", "20px")
        .on("mouseover", highlightHandler)
        .on("mouseleave", unhighlightHandler);
}

export function plot(
    transactions: TransactionsByCategoryResponse,
    width: number,
    height: number
): SVGGElement {
    const plotWidth = width - Y_AXIS_WIDTH - LEGEND_WIDTH;
    const plotHeight = height - X_AXIS_HEIGHT;

    const stackedData = stack(transactions.categories, transactions.amounts);
    const maxHeight = Math.max(...stackedData.map(row => Math.max(...row.map(coord => coord[1]))));
    const colorMap = buildColorMap(transactions.categories.length);

    const xScale = d3.scaleTime()
        .domain([transactions.dates[0] as Date, transactions.dates[transactions.dates.length - 1] as Date])
        .range([0, plotWidth]);

    const yScale = d3.scaleLinear()
        .domain([0, maxHeight])
        .range([plotHeight, 0]);

    const svg = d3.create("svg")
        .attr("viewBox", `${-Y_AXIS_WIDTH}, 0, ${width}, ${height}`)
        .attr("preserveAspectRatio", "xMidYMid meet");

    // Append axis ticks
    const xAxis = svg.append("g")
        .attr("transform", `translate(0, ${plotHeight})`)
        .call(d3.axisBottom(xScale).ticks(X_TICKS));

    // This is automatically right aligned to x=0
    svg.append("g")
        .call(d3.axisLeft(yScale).ticks(Y_TICKS));

    drawPlotArea(svg, transactions.dates, stackedData, colorMap, xScale, yScale, xAxis, plotWidth, plotHeight);
    drawLegend(svg, transactions.categories, colorMap, plotWidth);

    return svg.node() as SVGSVGElement;
}
