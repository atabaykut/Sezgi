from django.db import models


class Project(models.Model):
    name = models.CharField(max_length=100)
    api_key = models.CharField(max_length=32)


class Check(models.Model):
    project = models.ForeignKey(Project, on_delete=models.CASCADE)
    name = models.CharField(max_length=100)
    status = models.CharField(max_length=10, default="new")


class Ping(models.Model):
    owner = models.ForeignKey(Check, on_delete=models.CASCADE)
    created = models.DateTimeField(auto_now_add=True)


class Channel(models.Model):
    project = models.ForeignKey(Project, on_delete=models.CASCADE)
    kind = models.CharField(max_length=20)
