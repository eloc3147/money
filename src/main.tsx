import {PrimeReactProvider} from 'primereact/api';
import React from 'react';
import ReactDOM from 'react-dom/client';

import './index.css';
import App from './money/app.tsx';

ReactDOM.createRoot(document.querySelector('#root')!).render((
    <React.StrictMode>
        <PrimeReactProvider>
            <App/>
        </PrimeReactProvider>
    </React.StrictMode>
));
