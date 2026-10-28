<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Course extends Model
{
    protected $fillable = ['title', 'capacity'];

    public function students()
    {
        return $this->belongsToMany(User::class, 'enrollments');
    }
}
