<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

class CreateCoursesTable extends Migration
{
    public function up()
    {
        Schema::create('courses', function (Blueprint $table) {
            $table->increments('id');
            $table->string('title');
            $table->integer('capacity')->default(10);
        });

        Schema::create('enrollments', function (Blueprint $table) {
            $table->unsignedInteger('course_id');
            $table->unsignedBigInteger('user_id');
            $table->foreign('course_id')->references('id')->on('courses')->onDelete('cascade');
            $table->foreign('user_id')->references('id')->on('users')->onDelete('CASCADE');
        });
    }
}
