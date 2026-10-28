<?php

namespace App\Http\Controllers;

class AdminController extends Controller
{
    public function index()
    {
        $this->authorize('admin-panel');

        return view('admin');
    }

    public function audit()
    {
        return view('audit');
    }
}
