<?php

use App\Http\Controllers\SettingsController;
use Illuminate\Support\Facades\Route;

Route::middleware('auth')->group(function () {
    Route::post('settings/change-password', [SettingsController::class, 'changePassword'])->name('change-password');
    Route::post('settings/api-tokens', [SettingsController::class, 'createToken'])->name('create-token');
    Route::post('settings/two-factor', [SettingsController::class, 'enableTwoFactor'])->name('enable-2fa');
    Route::post('settings/profile', [SettingsController::class, 'updateProfile'])->name('update-profile');
});
