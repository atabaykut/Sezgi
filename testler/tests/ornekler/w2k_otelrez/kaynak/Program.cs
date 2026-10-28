// Kaynaksız keşif test örneği: yayımlanmış hâli (yayin/) kaynağı olmadan incelenir.
// Kaynaktaki gerçekler (beklenen olgular):
//  G1 kimlik: OtelRez 2.3.1, net8.0, "Ornek Otelcilik" / "OtelRez Rezervasyon"
//  G2 tipler: Rol (enum), Rezervasyon, IptalKurali, Depo, Program
//  G3 iptal kuralı: girişe 48 saatten az kala iptal %50 ceza
//  G4 rol kuralı: yalnızca Resepsiyon ve Yonetici başkasının rezervasyonunu iptal eder
//  G5 SQL: UPDATE rezervasyon / SELECT ... FROM oda
//  G6 P/Invoke: e_sqlite3!sqlite3_open
//  G7 çağrı: Main -> IptalKurali.IptalEt
//  G8 şema (otel.db): CHECK, FOREIGN KEY, indeks, tetikleyici, görünüm
using System;
using System.Runtime.InteropServices;

namespace OtelRez
{
    public enum Rol { Misafir, Resepsiyon, Yonetici }

    public sealed class Rezervasyon
    {
        public long Id { get; init; }
        public int OdaNo { get; init; }
        public DateTime Giris { get; init; }
        public decimal Tutar { get; init; }
        public long SahipId { get; init; }
        public string Durum { get; set; } = "aktif";
    }

    public static class IptalKurali
    {
        public const double EsikSaat = 48;
        public const decimal CezaOrani = 0.5m;

        /// <summary>İptal eder ve kesilecek cezayı döner.</summary>
        public static decimal IptalEt(Rezervasyon r, Rol rol, long isteyenId, DateTime simdi)
        {
            if (rol == Rol.Misafir && r.SahipId != isteyenId)
                throw new UnauthorizedAccessException("yalnizca kendi rezervasyonunuz");
            if (r.Durum != "aktif")
                throw new InvalidOperationException("rezervasyon aktif degil");
            r.Durum = "iptal";
            return (r.Giris - simdi).TotalHours < EsikSaat ? r.Tutar * CezaOrani : 0m;
        }
    }

    public static class Depo
    {
        public const string IptalSql = "UPDATE rezervasyon SET durum = 'iptal' WHERE id = $id";
        public const string OdaSql = "SELECT no, tip, fiyat FROM oda WHERE no = $no";

        [DllImport("e_sqlite3", EntryPoint = "sqlite3_open", CharSet = CharSet.Ansi)]
        private static extern int sqlite3_open(string dosya, out IntPtr db);

        public static bool Ac(string dosya) => sqlite3_open(dosya, out _) == 0;

        public static string IptalKomutu() => "UPDATE rezervasyon SET durum = 'iptal' WHERE id = " + "$id";
    }

    public static class Program
    {
        public static int Main(string[] args)
        {
            var r = new Rezervasyon { Id = 1, OdaNo = 101, Giris = DateTime.UtcNow.AddHours(24), Tutar = 1000m, SahipId = 7 };
            var ceza = IptalKurali.IptalEt(r, Rol.Misafir, 7, DateTime.UtcNow);
            Console.WriteLine($"ceza={ceza} sql={Depo.IptalKomutu()}");
            return 0;
        }
    }
}
