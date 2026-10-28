<?php

namespace App\Http\Controllers;

use App\Permissions\Permission;

abstract class Controller
{
    protected function showPermissionError(): void
    {
        abort(403);
    }

    protected function checkPermission(string|Permission $permission): void
    {
        if (!user()->can($permission)) {
            $this->showPermissionError();
        }
    }

    protected function checkOwnablePermission(string|Permission $permission, $ownable): void
    {
        if (!userCan($permission, $ownable)) {
            $this->showPermissionError();
        }
    }

    protected function getImageValidationRules(): array
    {
        return ['image_extension', 'mimes:jpeg,png,gif', 'max:' . (config('app.upload_limit') * 1000)];
    }
}
