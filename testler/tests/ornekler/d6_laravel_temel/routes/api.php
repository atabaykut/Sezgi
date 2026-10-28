<?php

use App\Http\Controllers\Api\CommentController;
use App\Http\Controllers\Api\OrderController;
use App\Http\Controllers\Api\PostController;
use App\Http\Controllers\Api\ProfileController;
use App\Http\Controllers\Api\AuthController;
use App\Http\Controllers\Api\ReportController;
use App\Http\Controllers\Api\TagController;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Route;

Route::get('/ping', fn () => response()->json(['ok' => true]));

Route::post('/login', [AuthController::class, 'login'])->name('auth.login');
Route::post('/register', [AuthController::class, 'register']);

Route::middleware('auth:sanctum')->group(function () {
    Route::get('/me', ProfileController::class)->name('me');
    Route::apiResource('posts', PostController::class);
    Route::apiResource('posts.comments', CommentController::class)->only(['index', 'store'])->shallow();
    Route::delete('/posts/{post}/force', [PostController::class, 'forceDestroy'])->whereNumber('post');

    Route::prefix('orders')->name('orders.')->group(function () {
        Route::post('/', [OrderController::class, 'store'])->name('store');
        Route::post('/{order}/ship', [OrderController::class, 'ship'])->name('ship');
    });

    Route::middleware('role:admin|editor')->group(function () {
        Route::get('/reports/daily', [ReportController::class, 'daily']);
        Route::put('/tags/{tag}', [TagController::class, 'update'])->middleware('can:update,tag');
    });

    Route::get('/team/stats', [ReportController::class, 'team'])->middleware('team');
    Route::post('/legacy', [ReportController::class, 'legacy'])->middleware('legacy.check');
});

Route::controller(TagController::class)->prefix('tags')->group(function () {
    Route::get('/', 'index');
    Route::get('/{tag}', 'show');
});

if (config('features.beta')) {
    Route::get('/beta', [ReportController::class, 'beta']);
}
