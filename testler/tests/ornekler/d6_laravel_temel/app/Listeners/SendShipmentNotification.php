<?php

namespace App\Listeners;

use App\Events\OrderShipped;
use App\Models\Comment;

class SendShipmentNotification
{
    public function handle(OrderShipped $event): void
    {
        if (! $event->order->user_id) {
            throw new \RuntimeException('kullanıcı yok');
        }
        Comment::where('order_id', $event->order->id)->delete();
    }
}
