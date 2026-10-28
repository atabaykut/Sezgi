from django.contrib.auth.decorators import login_required
from django.http import HttpResponse, HttpResponseNotAllowed, JsonResponse
from django.views.decorators.http import require_GET, require_http_methods, require_POST
from rest_framework.decorators import api_view

from izleme.decorators import allow_methods, api_key_required, timed, token_auth
from izleme.models import Channel, Check, Ping


@allow_methods("GET")
@api_key_required
def list_checks(request):
    return JsonResponse({"checks": [c.name for c in Check.objects.filter(project=request.project)]})


@allow_methods("POST")
@api_key_required
def pause(request, code):
    check = Check.objects.get(id=code)
    check.status = "paused"
    check.save()
    return JsonResponse({"status": "paused"})


@allow_methods("POST")
@api_key_required
def resume(request, code):
    check = Check.objects.get(id=code)
    check.status = "up"
    check.save()
    return JsonResponse({"status": "up"})


@allow_methods("POST", "DELETE")
@api_key_required
def single(request, code):
    check = Check.objects.get(id=code)
    if request.method == "DELETE":
        check.delete()
        return HttpResponse(status=204)
    check.name = request.json.get("name", check.name)
    check.save()
    return JsonResponse({})


@token_auth
def badges(request):
    return JsonResponse({"badges": []})


@timed
def ping(request, code):
    Ping.objects.create(owner_id=code)
    return HttpResponse("OK")


@require_POST
@login_required
def remove_check(request, code):
    Check.objects.filter(id=code).delete()
    return HttpResponse(status=204)


@require_GET
@login_required
def checks_page(request):
    return JsonResponse({})


@require_http_methods(["GET", "POST"])
@login_required
def channels(request):
    if request.method == "POST":
        Channel.objects.create(project_id=1, kind=request.POST["kind"])
    return JsonResponse({})


@login_required
def send_test_notification(request, code):
    if request.method != "POST":
        return HttpResponseNotAllowed(["POST"])
    channel = Channel.objects.get(id=code)
    return JsonResponse({"kind": channel.kind})


@api_view(["POST"])
def transfer_check(request, code):
    return JsonResponse({})


def oups(request):
    raise RuntimeError("expected: some exception")


@api_key_required
def create_check(request):
    check = Check.objects.create(project=request.project, name="new")
    return JsonResponse({"id": check.id}, status=201)


@timed
def checks(request):
    if request.method == "POST":
        return create_check(request)
    return list_checks(request)
