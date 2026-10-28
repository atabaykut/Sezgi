<?php

namespace App\Policies;

class CoursePolicy
{
    public function update($user, $course)
    {
        return $user->hasRole('teacher');
    }
}
