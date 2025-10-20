import React from 'react';
import ReactDOM from 'react-dom/client';
import {PrimeReactProvider} from 'primereact/api';
import App from './app.tsx';
import './index.css';

ReactDOM.createRoot(document.querySelector('#root')!).render((
    <React.StrictMode>
        <PrimeReactProvider>
            <App/>
        </PrimeReactProvider>
    </React.StrictMode>
));
