from functools import wraps

from django.http import HttpResponse, JsonResponse

from izleme.models import Project


def error(msg, status=400):
    return JsonResponse({"error": msg}, status=status)


def api_key_required(f):
    @wraps(f)
    def wrapper(request, *args, **kwds):
        api_key = request.headers.get("X-Api-Key", "")
        if len(api_key) != 32:
            return error("missing api key", 401)
        project = Project.objects.filter(api_key=api_key).first()
        if project is None:
            return error("wrong api key", 401)
        request.project = project
        return f(request, *args, **kwds)

    return wrapper


def token_auth(f):
    @wraps(f)
    def wrapper(request, *args, **kwds):
        token = request.META.get("HTTP_AUTHORIZATION", "")
        if not token.startswith("Bearer "):
            return HttpResponse(status=403)
        return f(request, *args, **kwds)

    return wrapper


def allow_methods(*methods):
    def decorator(f):
        @wraps(f)
        def wrapper(request, *args, **kwds):
            if request.method == "OPTIONS":
                return HttpResponse(status=204)
            if request.method in methods:
                return f(request, *args, **kwds)
            return HttpResponse(status=405)

        return wrapper

    return decorator


def timed(f):
    @wraps(f)
    def wrapper(request, *args, **kwds):
        return f(request, *args, **kwds)

    return wrapper
