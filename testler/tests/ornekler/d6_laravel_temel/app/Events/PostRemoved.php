<?php

namespace App\Events;

use Illuminate\Foundation\Events\Dispatchable;

class PostRemoved
{
    use Dispatchable;

    public function __construct(public int $id)
    {
    }
}
