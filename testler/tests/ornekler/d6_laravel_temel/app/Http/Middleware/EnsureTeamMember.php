<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;

class EnsureTeamMember
{
    public function handle(Request $request, Closure $next)
    {
        if (! $request->user() || ! $request->user()->team_id) {
            abort(403, 'Takım üyesi değil');
        }

        return $next($request);
    }
}
