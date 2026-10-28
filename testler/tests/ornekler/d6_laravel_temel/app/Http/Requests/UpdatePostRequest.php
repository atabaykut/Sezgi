<?php

namespace App\Http\Requests;

class UpdatePostRequest extends StorePostRequest
{
    public function rules(): array
    {
        return array_merge(parent::rules(), [
            'title' => 'sometimes|string|max:200',
            'pinned' => 'boolean',
        ]);
    }
}
