<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;

class CheckUserHasPermission
{
    public function handle(Request $request, Closure $next, string $permission)
    {
        if (!user()->can($permission)) {
            abort(403);
        }

        return $next($request);
    }
}
