<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Resources\UserResource;
use Illuminate\Http\Request;

class ProfileController extends Controller
{
    /**
     * Oturumdaki kullanıcı.
     */
    public function __invoke(Request $request): UserResource
    {
        return new UserResource($request->user());
    }
}
