<?php

namespace App\Http\Controllers;

use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Hash;

class SettingsController extends Controller
{
    public function changePassword(Request $request): RedirectResponse
    {
        $request->user()->update(['password' => Hash::make($request->input('new_password'))]);

        return back();
    }

    public function createToken(Request $request): RedirectResponse
    {
        $request->user()->tokens()->create(['name' => $request->input('name')]);

        return back();
    }

    public function enableTwoFactor(Request $request): RedirectResponse
    {
        $request->user()->update(['two_factor_enabled' => true]);

        return back();
    }

    public function updateProfile(Request $request): RedirectResponse
    {
        $request->user()->update(['name' => $request->input('name')]);

        return back();
    }
}
