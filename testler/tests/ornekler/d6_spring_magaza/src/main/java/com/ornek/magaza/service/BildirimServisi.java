package com.ornek.magaza.service;

import com.ornek.magaza.event.UrunOlusturuldu;
import com.ornek.magaza.repo.UrunRepository;
import org.springframework.context.event.EventListener;
import org.springframework.kafka.core.KafkaTemplate;
import org.springframework.scheduling.annotation.Async;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;

@Service
public class BildirimServisi {
    private final UrunRepository urunRepository;
    private final KafkaTemplate<String, String> kafkaTemplate;

    public BildirimServisi(UrunRepository urunRepository, KafkaTemplate<String, String> kafkaTemplate) {
        this.urunRepository = urunRepository;
        this.kafkaTemplate = kafkaTemplate;
    }

    @Async
    public void gonder(String mesaj) {
        kafkaTemplate.send("bildirimler", mesaj);
    }

    @EventListener
    public void urunOlustu(UrunOlusturuldu olay) {
        gonder("urun " + olay.id());
    }

    @Scheduled(cron = "0 0 * * * *")
    public void temizle() {
        urunRepository.stoksuzlariSil();
    }
}
