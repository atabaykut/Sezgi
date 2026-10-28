from sqlmodel import Session, select

from app.models import User


def get_user_by_email(*, session: Session, email: str) -> User | None:
    statement = select(User).where(User.email == email)
    return session.exec(statement).first()


def update_user(*, session: Session, db_user: User, email: str) -> User:
    db_user.email = email
    session.add(db_user)
    session.commit()
    return db_user
