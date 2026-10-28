<?php

namespace App\Permissions;

enum Permission: string
{
    case SettingsManage = 'settings-manage';
    case BookCreateAll = 'book-create-all';
    case BookUpdate = 'book-update';
    case ContentExport = 'content-export';

    public function middleware(): string
    {
        return 'can:' . $this->value;
    }
}
