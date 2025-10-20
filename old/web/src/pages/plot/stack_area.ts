function offset(series: StackRow[]): void {
    let s0: StackRow, s1: StackRow = series[0] as StackRow;
    const colCount = series.length,
        rowCount = s1.length;
    for (let rowIdx = 1; rowIdx < colCount; rowIdx += 1) {
        s0 = s1;
        s1 = series[rowIdx] as StackRow;
        for (let colIdx = 1; colIdx < rowCount; colIdx += 1) {
            const s0val = s0[colIdx] as [number, number];
            const s1val = s1[colIdx] as [number, number];
            let newVal;
            if (isNaN(s0val[1])) {
                newVal = s0val[0];
            } else {
                newVal = s0val[1];
            }
            s1val[0] = newVal;
            s1val[1] += newVal;
        }
    }
}

export type Coordinate = [number, number];
export interface StackCoordinate extends Coordinate {
    data: number[];
}

export interface StackRow extends Array<StackCoordinate> {
    key: string;
    index: number;
}

export type StackData = StackRow[];

export function stack(keys: string[], data: number[][]): StackRow[] {
    const series: StackRow[] = Array.from(keys, (key, idx) => {
        const values = [] as unknown as StackRow;
        values.key = key;
        values.index = idx;
        return values;
    });

    let rowIdx = 0;
    for (const row of data) {
        for (let colIdx = 0; colIdx < series.length; colIdx += 1) {
            const value = [0, row[colIdx] as number] as StackCoordinate;
            value.data = row;
            (series[colIdx] as StackRow)[rowIdx] = value;
        }
        rowIdx += 1;
    }

    offset(series);

    return series;
}
