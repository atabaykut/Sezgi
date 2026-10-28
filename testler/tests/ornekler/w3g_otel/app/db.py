from sqlmodel import Session


def oturum() -> Session:
    return Session()
