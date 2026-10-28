package com.ornek.kitaplik.config

import org.aspectj.lang.ProceedingJoinPoint
import org.aspectj.lang.annotation.Around
import org.aspectj.lang.annotation.Aspect
import org.springframework.stereotype.Component

@Target(AnnotationTarget.FUNCTION)
@Retention(AnnotationRetention.RUNTIME)
annotation class ApiAnahtari(val zorunlu: Boolean = true)

@Aspect
@Component
class ApiAnahtariKesiti {
    @Around("@annotation(com.ornek.kitaplik.config.ApiAnahtari)")
    fun denetle(nokta: ProceedingJoinPoint): Any? {
        return nokta.proceed()
    }
}
