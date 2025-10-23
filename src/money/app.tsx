import 'primeicons/primeicons.css';
import {Column} from 'primereact/column';
import {DataTable} from 'primereact/datatable';
import 'primereact/resources/themes/lara-dark-teal/theme.css';
import {useEffect, useState} from 'react';

import {fetchTransactions, Transaction} from './api.ts';
import './app.css';

function TransactionTable() {
    const [transactionFilter, setTransactionFilter] = useState(0);
    const [transactions, setTransactions] = useState<Transaction[]>([]);

    useEffect(() => {
        async function applyTransactions(): Promise<void> {
            setTransactions(await fetchTransactions());
        }

        void applyTransactions();
    }, [transactionFilter]);

    return (
        <DataTable
            showGridlines
            value={transactions}
            tableStyle={{minWidth: '50rem'}}
            emptyMessage='No transactions found'
            sortField='date'
            sortOrder={-1}
        >
            <Column sortable field='account' header='Account'/>
            <Column sortable field='base_category' header='Base Category'/>
            <Column sortable field='category' header='Category'/>
            <Column sortable field='source_category' header='Source Category'/>
            <Column sortable field='income' header='Income'/>
            <Column sortable field='transaction_type' header='Type'/>
            <Column sortable field='date' header='Date'/>
            <Column sortable field='amount' header='Amount'/>
            <Column sortable field='transaction_id' header='Transaction ID'/>
            <Column sortable field='name' header='Name'/>
            <Column sortable field='memo' header='Memo'/>
        </DataTable>
    );
}

function App() {
    return (
        <div className='App'>
            <h1>Money</h1>

            <TransactionTable/>
        </div>
    );
}

export default App;
