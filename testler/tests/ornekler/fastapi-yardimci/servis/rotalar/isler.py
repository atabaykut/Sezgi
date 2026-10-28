from fastapi import APIRouter

isler_router = APIRouter(prefix="/isler")
# Yol metin toplamayla kurulur (Airflow: task_instances_prefix + "/{task_id}")
denemeler_oneki = "/{is_id}/denemeler"


@isler_router.get("")
def isleri_listele():
    return []


@isler_router.delete("/{is_id}")
def isi_sil(is_id: str):
    return None


@isler_router.get(denemeler_oneki + "/{deneme_no}")
def denemeyi_getir(is_id: str, deneme_no: int):
    return {}
