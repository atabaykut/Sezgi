<?php

use Illuminate\Support\Facades\Route;

Route::group(['prefix' => 'v2', 'middleware' => 'auth:api', 'as' => 'v2.'], function () {
    Route::get('courses', 'CourseController@index')->name('courses.index');
    Route::post('courses/{id}/enroll', 'CourseController@enroll');
    Route::match(['put', 'patch'], 'courses/{id}', 'CourseController@update');
    Route::resources([
        'teachers' => TeacherController::class,
    ]);
    Route::group(['middleware' => 'admin', 'namespace' => 'Admin'], function () {
        Route::delete('users/{user}', 'UserController@destroy')->middleware('can:manage-users');
    });
});

Route::any('webhook', 'WebhookController@handle')->withoutMiddleware('throttle:api');
