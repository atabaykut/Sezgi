<?php

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use Illuminate\Http\Request;

class SessionController extends Controller
{
    public function create()
    {
        return view('auth.login');
    }

    public function store(Request $request)
    {
        $request->validate(['email' => 'required|email', 'password' => 'required']);

        return redirect('/dashboard');
    }

    public function destroy(Request $request)
    {
        auth()->logout();

        return redirect('/');
    }
}
