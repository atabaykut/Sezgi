<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Order extends Model
{
    protected $table = 'shop_orders';

    protected $fillable = ['user_id', 'total', 'status'];
}
