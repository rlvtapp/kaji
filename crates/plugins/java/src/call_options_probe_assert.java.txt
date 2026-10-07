        String expectedScope;java.time.Duration expectedTimeout;String expectedAuth;
        void assertRequest(HttpRequest request) {
            calls++;check(request.headers().firstValue("X-Scope").orElse(null)==null?expectedScope==null:request.headers().firstValue("X-Scope").get().equals(expectedScope),"scope header");check(request.timeout().orElseThrow().equals(expectedTimeout),"native JDK timeout");check(request.headers().allValues("Authorization").equals(List.of(expectedAuth)),"explicit scoped auth ownership");
        }
