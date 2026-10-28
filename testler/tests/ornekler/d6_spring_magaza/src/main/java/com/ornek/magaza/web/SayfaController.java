package com.ornek.magaza.web;

import org.springframework.context.annotation.Profile;
import org.springframework.stereotype.Controller;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestMethod;

@Controller
@Profile("dev")
@RequestMapping(value = "/eski")
public class SayfaController {

    @RequestMapping(path = "/sayfa", method = {RequestMethod.GET, RequestMethod.POST})
    public String sayfa() {
        return "sayfa";
    }

    @RequestMapping("/her")
    public String her() {
        return "her";
    }
}
