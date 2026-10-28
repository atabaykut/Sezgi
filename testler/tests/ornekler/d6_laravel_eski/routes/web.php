<?php

use Illuminate\Support\Facades\Route;

Route::resource('courses', 'CourseController')->except(['destroy']);
Route::get('/about', 'PageController@about');
