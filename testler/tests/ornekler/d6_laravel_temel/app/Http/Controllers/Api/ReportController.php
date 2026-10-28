<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use Illuminate\Support\Facades\DB;

class ReportController extends Controller
{
    public function daily()
    {
        return DB::select('select count(*) from posts');
    }

    public function team()
    {
        return ['ok' => true];
    }

    public function legacy()
    {
        return ['ok' => true];
    }

    public function beta()
    {
        return ['beta' => true];
    }
}
