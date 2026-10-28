<?php

namespace App\Providers;

use App\Events\StudentEnrolled;
use App\Listeners\NotifyTeacher;
use Illuminate\Foundation\Support\Providers\EventServiceProvider as ServiceProvider;

class EventServiceProvider extends ServiceProvider
{
    protected $listen = [
        StudentEnrolled::class => [
            NotifyTeacher::class,
        ],
    ];
}
