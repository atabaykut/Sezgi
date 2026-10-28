<?php

namespace App\Http\Controllers\Api;

use App\Events\OrderShipped;
use App\Http\Controllers\Controller;
use App\Jobs\ProcessOrder;
use App\Models\Order;
use Illuminate\Http\Request;

class OrderController extends Controller
{
    public function store(Request $request)
    {
        $order = Order::create(['user_id' => $request->user()->id, 'total' => $request->integer('total')]);
        ProcessOrder::dispatch($order);

        return $order;
    }

    public function ship(Order $order)
    {
        $order->update(['status' => $order->paid ? 'shipped' : 'pending']);
        event(new OrderShipped($order));

        return $order;
    }
}
