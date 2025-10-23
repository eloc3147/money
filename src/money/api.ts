import {invoke} from '@tauri-apps/api/core';

export type Transaction = {
    account: string;
    base_category: string;
    category: string;
    source_category: string | undefined;
    income: boolean;
    transaction_type: string;
    date: string;
    amount: number;
    transaction_id: string | undefined;
    name: string;
    memo: string | undefined;
};

export async function fetchTransactions(): Promise<Transaction[]> {
    return invoke('fetch_transactions');
}
