<?php

namespace App\Listeners;

use App\Events\StudentEnrolled;

class NotifyTeacher
{
    public function handle(StudentEnrolled $event)
    {
        abort_if($event->course->capacity < 1, 409);
    }
}
