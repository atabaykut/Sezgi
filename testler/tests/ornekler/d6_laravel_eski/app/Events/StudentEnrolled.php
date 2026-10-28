<?php

namespace App\Events;

class StudentEnrolled
{
    public function __construct(public $course)
    {
    }
}
