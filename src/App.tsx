import {useState} from 'react';
import {Button} from 'primereact/button';
import {InputText} from 'primereact/inputtext';
import 'primereact/resources/themes/lara-dark-teal/theme.css';
import 'primeicons/primeicons.css';
import './App.css';

function App() {
    const [count, setCount] = useState(0);

    return (
        <div className='App'>
            <h1>Money</h1>

            <div className='card'>
                <Button
                    icon='pi pi-plus'
                    className='mr-2'
                    label='Increment'
                    onClick={() => {
                        setCount(count => count + 1);
                    }}
                />
                <InputText value={count as unknown as string}/>
            </div>
        </div>
    );
}

export default App;
