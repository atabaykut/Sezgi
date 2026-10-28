from fastapi import APIRouter


def controller(router: APIRouter, *urls: str):
    """Sınıf tabanlı görünüm: sınıftaki router uçları örneğe bağlanır."""

    def decorator(cls):
        return cls

    return decorator
