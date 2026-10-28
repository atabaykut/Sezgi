const axios = require('axios');
const nodemailer = require('nodemailer');
const { Kafka } = require('kafkajs');
const prisma = require('../prisma');

const ODEME_ADRESI = 'https://api.odeme.example.com/v1/charges';
const posta = nodemailer.createTransport({ host: process.env.SMTP_HOST });
const producer = new Kafka({ brokers: ['kafka:9092'] }).producer();

const odemeAl = async (tutar) => {
  return axios.post(ODEME_ADRESI, { tutar });
};

const siparisVer = async (girdi) => {
  if (!girdi.urunId) {
    throw new Error('urun gerekli');
  }
  const siparis = await prisma.siparis.create({ data: girdi });
  await odemeAl(girdi.tutar);
  await posta.sendMail({ to: girdi.eposta, subject: 'Siparis' });
  await producer.send({ topic: 'siparis-olustu', messages: [{ value: String(siparis.id) }] });
  return siparis;
};

const ucusGetir = async (no) => {
  const cevap = await fetch(`${process.env.UCUS_API_URL}/flights/${no}`);
  return cevap.json();
};

const siparisSil = async (id) => {
  await prisma.siparis.delete({ where: { id } });
};

const siparisGuncelle = async (id, girdi) => {
  const siparis = await prisma.siparis.findFirst({ where: { id } });
  if (siparis.durum === 'kapandi') {
    throw new Error('kapanan siparis degismez');
  }
  return prisma.siparis.update({ where: { id }, data: girdi });
};

const notEkle = async (girdi) => prisma.not.create({ data: girdi });

const notSil = async (id) => {
  await prisma.not.update({ where: { id }, data: { deletedAt: new Date() } });
};

const notGeriYukle = async (id) => prisma.not.update({ where: { id }, data: { deletedAt: null } });

const yansit = async (adres) => {
  const cevap = await fetch(adres, { method: 'POST' });
  return cevap.status;
};

const notIsaretle = async (id) => prisma.not.update({ where: { id }, data: { isaretli: true } });

const notIsaretKaldir = async (id) => prisma.not.update({ where: { id }, data: { isaretli: false } });

const notGuncelle = async (id, girdi) => {
  const eski = await prisma.not.findFirst({ where: { id } });
  await notRevisionRepo.create({ notId: id, metin: eski.metin });
  return prisma.not.update({ where: { id }, data: girdi });
};

const notSurumuneDon = async (id, surum) => prisma.not.update({ where: { id }, data: { surum } });

module.exports = { notIsaretle, notIsaretKaldir, notGuncelle, notSurumuneDon, siparisVer, ucusGetir, siparisSil, siparisGuncelle, notEkle, notSil, notGeriYukle, yansit };
