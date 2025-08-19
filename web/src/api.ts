async function apiRequest(endpoint: string): Promise<object> {
    const resp = await fetch(`/api/${endpoint}`);
    if (!resp.ok) {
        throw new Error(`Loading data failed with code: ${resp.status}`);
    }

    return await resp.json();
}

export interface TransactionsResponse {
    categories: string[];
    dates: Date[];
    amounts: number[][];
}

export async function loadExpenses(): Promise<TransactionsResponse> {
    const resp = await apiRequest("expenses") as any;
    return {
        categories: resp.categories,
        dates: Array.from(resp.dates, (dateStr: string) => new Date(dateStr)),
        amounts: resp.amounts,
    };
}

export async function loadIncome(): Promise<TransactionsResponse> {
    const resp = await apiRequest("income") as any;
    return {
        categories: resp.categories,
        dates: Array.from(resp.dates, (dateStr: string) => new Date(dateStr)),
        amounts: resp.amounts,
    };
}
