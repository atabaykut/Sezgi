from sqlalchemy import Boolean, Column, Integer
from sqlalchemy.orm import declarative_base

Base = declarative_base()


class Item(Base):
    __tablename__ = "items"
    id = Column(Integer, primary_key=True)
    done = Column(Boolean)


class Log(Base):
    __tablename__ = "logs"
    id = Column(Integer, primary_key=True)
