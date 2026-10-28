<?php

namespace App\Http\Controllers;

use App\Events\StudentEnrolled;
use App\Models\Course;
use Illuminate\Http\Request;

class CourseController extends Controller
{
    public function __construct()
    {
        $this->middleware('auth')->except(['index', 'show']);
    }

    public function index()
    {
        return Course::all();
    }

    public function create()
    {
        return view('courses.create');
    }

    public function store(Request $request)
    {
        $this->validate($request, ['title' => 'required|max:120', 'capacity' => 'integer|min:1']);

        return Course::create($request->all());
    }

    public function show($id)
    {
        return Course::findOrFail($id);
    }

    public function edit($id)
    {
        return view('courses.edit');
    }

    public function update(Request $request, $id)
    {
        $course = Course::findOrFail($id);
        $this->authorize('update', $course);
        $course->update($request->only('title'));

        return $course;
    }

    public function enroll(Request $request, $id)
    {
        $course = Course::findOrFail($id);
        $course->students()->attach($request->user()->id);
        event(new StudentEnrolled($course));

        return response()->json(['enrolled' => true]);
    }
}
