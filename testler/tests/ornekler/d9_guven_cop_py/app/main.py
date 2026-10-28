from fastapi import Depends, FastAPI
from sqlalchemy import delete

from app.models import Item, Log

app = FastAPI()


def get_db():
    yield None


@app.post("/items/purge")
def purge_items(db=Depends(get_db)):
    db.query(Item).filter(Item.done == True).delete()
    db.commit()
    return {"ok": True}


@app.post("/logs/clear")
def clear_logs(db=Depends(get_db)):
    db.execute(delete(Log).where(Log.id > 0))
    db.commit()
    return {"ok": True}


@app.post("/items/clear")
def clear_items(db=Depends(get_db)):
    db.query(Item).filter(
        Item.done == True,
    ).delete()
    db.commit()
    return {"ok": True}
