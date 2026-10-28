<?php

use App\Models\User;

function user(): User
{
    return auth()->user() ?: User::getGuest();
}

function userCan(string $permission, $ownable = null): bool
{
    return user()->can($permission);
}
