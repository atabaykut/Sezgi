<?php

namespace Tests\Feature;

use Illuminate\Support\Facades\Route;

class ExampleTest
{
    public function test_x()
    {
        Route::get('/test-only', fn () => 1);
    }
}
