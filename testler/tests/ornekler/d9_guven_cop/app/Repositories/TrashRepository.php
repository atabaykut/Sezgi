<?php

namespace App\Repositories;

use App\Models\Link;

class TrashRepository
{
    public static function delete(string $model): bool
    {
        $entries = Link::onlyTrashed()->get();

        foreach ($entries as $entry) {
            $entry->forceDelete();
        }

        return true;
    }
}
