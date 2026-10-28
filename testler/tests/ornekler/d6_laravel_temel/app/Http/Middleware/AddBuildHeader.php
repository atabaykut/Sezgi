<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;

class AddBuildHeader
{
    public function handle(Request $request, Closure $next)
    {
        $response = $next($request);
        $response->headers->set('X-Build', '1');

        return $response;
    }
}
