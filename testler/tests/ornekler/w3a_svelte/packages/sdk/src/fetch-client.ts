/**
 * DO NOT MODIFY - This file has been generated using oazapfts.
 */
import * as Oazapfts from "@oazapfts/runtime";
import * as QS from "@oazapfts/runtime/query";
const oazapfts = Oazapfts.runtime({});

export function varliklariSil({ dto }: { dto: unknown }, opts?: Oazapfts.RequestOpts) {
    return oazapfts.ok(oazapfts.fetchText("/varliklar", oazapfts.json({
        ...opts,
        method: "DELETE",
        body: dto
    })));
}

export function varlikGetir({ id, key }: { id: string; key?: string }, opts?: Oazapfts.RequestOpts) {
    return oazapfts.ok(oazapfts.fetchJson<{ status: 200; data: unknown }>(`/varliklar/${encodeURIComponent(id)}${QS.query(QS.explode({ key }))}`, {
        ...opts
    }));
}
