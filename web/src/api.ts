async function apiRequest(endpoint: string): Promise<object> {
    const resp = await fetch(`/api/${endpoint}`);
    if (!resp.ok) {
        throw new Error(`Loading data failed with code: ${resp.status}`);
    }

    return await resp.json();
}

export type TransactionsResponse = [
    string,         // Account
    string,         // Base Category
    string,         // Category
    string | null,  // Source Category
    boolean,        // Income
    string,         // Transaction Type
    string,         // Date Str
    number,         // Amount
    string | null,  // Transaction Id
    string,         // Name
    string | null,  // Memo
][];

export async function loadTransactions(): Promise<TransactionsResponse> {
    return await apiRequest("transactions") as TransactionsResponse;
}

export interface TransactionsByCategoryResponse {
    categories: string[];
    dates: Date[];
    amounts: number[][];
}

export async function loadExpenses(): Promise<TransactionsByCategoryResponse> {
    const resp = await apiRequest("expenses") as any;
    return {
        categories: resp.categories,
        dates: Array.from(resp.dates, (dateStr: string) => new Date(dateStr)),
        amounts: resp.amounts,
    };
}

export async function loadIncome(): Promise<TransactionsByCategoryResponse> {
    const resp = await apiRequest("income") as any;
    return {
        categories: resp.categories,
        dates: Array.from(resp.dates, (dateStr: string) => new Date(dateStr)),
        amounts: resp.amounts,
    };
}
