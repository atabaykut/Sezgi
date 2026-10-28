<?php

namespace App\Http\Controllers;

use App\Permissions\Permission;
use Illuminate\Http\Request;

class SettingsController extends Controller
{
    public function index()
    {
        $this->checkPermission(Permission::SettingsManage);

        return view('settings.index');
    }

    public function update(Request $request)
    {
        $this->checkPermission('settings-manage');
        $request->validate(['app_name' => ['required', 'string', 'max:100']]);

        return redirect('/settings');
    }
}
