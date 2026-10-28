<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\User;

class UserController extends Controller
{
    public function destroy(User $user)
    {
        $user->delete();

        return response()->noContent();
    }
}
