<?php

namespace App\Http\Controllers;

use Laravel\Socialite\Facades\Socialite;

class SocialiteController extends Controller
{
    public function redirect(string $provider)
    {
        if (!config('services.' . $provider . '.enabled')) {
            abort(403, 'provider disabled');
        }

        return Socialite::driver($provider)->redirect();
    }

    public function callback(string $provider)
    {
        if (!config('services.' . $provider . '.enabled')) {
            abort(403, 'provider disabled');
        }
        $user = Socialite::driver($provider)->user();

        return redirect()->intended('/');
    }
}
