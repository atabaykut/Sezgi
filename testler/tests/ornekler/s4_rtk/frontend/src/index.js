import React from 'react';
import ReactDOM from 'react-dom/client';
import { createBrowserRouter, createRoutesFromElements, Route, RouterProvider } from 'react-router-dom';
import App from './App';
import ProductListScreen from './screens/admin/ProductListScreen';

const router = createBrowserRouter(
  createRoutesFromElements(
    <Route path='/' element={<App />}>
      <Route path='/admin/productlist' element={<ProductListScreen />} />
    </Route>
  )
);

ReactDOM.createRoot(document.getElementById('root')).render(<RouterProvider router={router} />);
