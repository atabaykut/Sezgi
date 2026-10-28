<?php

use App\Http\Controllers\AdminController;
use App\Http\Controllers\BookController;
use App\Http\Controllers\ExportController;
use App\Http\Controllers\SettingsController;
use Illuminate\Support\Facades\Route;

Route::get('/books', [BookController::class, 'index']);

Route::middleware('auth')->group(function () {
    Route::get('/settings', [SettingsController::class, 'index']);
    Route::put('/settings', [SettingsController::class, 'update']);
    Route::post('/books', [BookController::class, 'store']);
    Route::put('/books/{book}', [BookController::class, 'update']);
    Route::get('/admin', [AdminController::class, 'index']);
    Route::get('/audit', [AdminController::class, 'audit'])->middleware('can:audit-view');
    Route::get('/export/{book}', [ExportController::class, 'show']);
});
