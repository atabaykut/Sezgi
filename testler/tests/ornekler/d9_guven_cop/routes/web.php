<?php

use App\Http\Controllers\SocialiteController;
use App\Http\Controllers\TrashController;
use Illuminate\Support\Facades\Route;

Route::get('auth/{provider}', [SocialiteController::class, 'redirect'])->name('auth.redirect');
Route::get('auth/{provider}/callback', [SocialiteController::class, 'callback'])->name('auth.callback');

Route::middleware('auth')->group(function () {
    Route::get('trash', [TrashController::class, 'index'])->name('get-trash');
    Route::post('trash/clear', [TrashController::class, 'clearTrash'])->name('clear-trash');
    Route::post('notes/purge', [TrashController::class, 'purgeNotes'])->name('purge-notes');
    Route::post('notes/clear-done', [TrashController::class, 'clearDoneNotes'])->name('clear-done-notes');
    Route::post('links/purge', [TrashController::class, 'purgeLinks'])->name('purge-links');
});
