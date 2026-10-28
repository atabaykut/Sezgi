<?php

namespace App\Services;

use App\Models\AuditLog;

class AuditService
{
    public function record(string $olay): void
    {
        AuditLog::create(['olay' => $olay]);
    }
}
