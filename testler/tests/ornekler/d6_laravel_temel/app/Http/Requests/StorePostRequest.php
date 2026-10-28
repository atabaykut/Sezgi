<?php

namespace App\Http\Requests;

use App\Enums\PostStatus;
use Illuminate\Foundation\Http\FormRequest;
use Illuminate\Validation\Rule;
use Illuminate\Validation\Rules\Enum;

class StorePostRequest extends FormRequest
{
    public function authorize(): bool
    {
        return $this->user() !== null;
    }

    public function rules(): array
    {
        return [
            'title' => 'required|string|max:200',
            'body' => ['required', 'string'],
            'status' => ['required', new Enum(PostStatus::class)],
            'published_at' => 'nullable|date',
            'tags' => 'array',
            'tags.*' => 'integer|exists:tags,id',
            'category' => ['sometimes', Rule::in(['news', 'blog'])],
        ];
    }
}
