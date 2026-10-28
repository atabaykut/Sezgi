ROOT_URLCONF = "nabiz.urls"
DEBUG = False

INSTALLED_APPS = [
    "django.contrib.auth",
    "rest_framework",
    "izleme",
]

MIDDLEWARE = [
    "django.contrib.sessions.middleware.SessionMiddleware",
    "django.contrib.auth.middleware.AuthenticationMiddleware",
]
