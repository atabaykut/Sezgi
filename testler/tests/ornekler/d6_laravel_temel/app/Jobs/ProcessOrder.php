<?php

namespace App\Jobs;

use App\Events\OrderShipped;
use App\Models\Order;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;

class ProcessOrder implements ShouldQueue
{
    use Dispatchable, Queueable;

    public function __construct(public Order $order)
    {
    }

    public function handle(): void
    {
        $this->order->update(['status' => 'processed']);
        OrderShipped::dispatch($this->order);
    }
}
