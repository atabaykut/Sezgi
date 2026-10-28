import django.db.models.deletion
from django.db import migrations, models


class Migration(migrations.Migration):
    initial = True
    dependencies = []

    operations = [
        migrations.CreateModel(
            name="Yazar",
            fields=[("id", models.BigAutoField(primary_key=True)), ("ad", models.CharField(max_length=100))],
        ),
        migrations.CreateModel(
            name="Rapor",
            fields=[
                ("id", models.BigAutoField(primary_key=True)),
                ("yazar", models.ForeignKey(on_delete=django.db.models.deletion.CASCADE, to="kitaplar.yazar")),
            ],
        ),
    ]
