from fastapi import APIRouter, Request

from app.deps import oauth

router = APIRouter(prefix="/auth")


@router.get("/oauth/callback")
async def oauth_callback(request: Request):
    client = oauth.create_client("oidc")
    token = await client.authorize_access_token(request)
    return {"token": token}


@router.get("/oauth")
async def oauth_login(request: Request):
    client = oauth.create_client("oidc")
    return await client.authorize_redirect(request, "/auth/oauth/callback")
