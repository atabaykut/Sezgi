<?php

namespace App\Models;

use Illuminate\Foundation\Auth\User as Authenticatable;

class User extends Authenticatable
{
    public static function getGuest(): self
    {
        return new self();
    }
}
