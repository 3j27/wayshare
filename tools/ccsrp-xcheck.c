// Client-side SRP oracle using the system corecrypto, configured the way
// ScreenSharing.framework does it (ccsrp_ctx_init, SHA-512, RFC 5054 2048, username "").
//
//   ccsrp-xcheck verifier <salt> <password>
//   ccsrp-xcheck client <salt> <B> <password> <seed> [M2]
//
// All arguments and outputs are hex. Build: clang -O1 -o ccsrp-xcheck ccsrp-xcheck.c
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct ccrng_state {
    int (*generate)(struct ccrng_state *rng, size_t outlen, void *out);
};
struct ccdigest_info;
typedef const void *ccsrp_const_gp_t;

extern const struct ccdigest_info *ccsha512_di(void);
extern ccsrp_const_gp_t ccsrp_gp_rfc5054_2048(void);
extern int ccsrp_ctx_init(void *srp, const struct ccdigest_info *di, ccsrp_const_gp_t gp);
extern int ccsrp_client_start_authentication(void *srp, struct ccrng_state *rng, void *A);
extern int ccsrp_client_process_challenge(void *srp, const char *username, size_t pwlen,
                                          const void *pw, size_t saltlen, const void *salt,
                                          const void *B, void *M);
extern bool ccsrp_client_verify_session(void *srp, const uint8_t *HAMK);
extern const void *ccsrp_get_session_key(void *srp, size_t *len);
extern const void *ccsrp_get_premaster_secret(void *srp);
extern int ccsrp_generate_verifier(void *srp, const char *username, size_t pwlen,
                                   const void *pw, size_t saltlen, const void *salt,
                                   void *verifier);

static uint32_t seed;

static int det_generate(struct ccrng_state *rng, size_t n, void *out) {
    (void)rng;
    uint8_t *p = out;
    uint32_t x = seed * 2654435761u + 1;
    for (size_t i = 0; i < n; i++) {
        x ^= x << 13, x ^= x >> 17, x ^= x << 5;
        p[i] = (uint8_t)x;
    }
    return 0;
}

static size_t unhex(const char *s, uint8_t **out) {
    size_t n = strlen(s) / 2;
    *out = malloc(n ? n : 1);
    for (size_t i = 0; i < n; i++) sscanf(s + 2 * i, "%2hhx", &(*out)[i]);
    return n;
}

static void hex(const char *label, const void *p, size_t n) {
    printf("%s=", label);
    for (size_t i = 0; i < n; i++) printf("%02x", ((const uint8_t *)p)[i]);
    printf("\n");
}

int main(int argc, char **argv) {
    static _Alignas(16) uint8_t ctx[1 << 16];
    ccsrp_ctx_init(ctx, ccsha512_di(), ccsrp_gp_rfc5054_2048());
    uint8_t *salt, *pw, *B, *m2;

    if (argc == 4 && !strcmp(argv[1], "verifier")) {
        size_t sl = unhex(argv[2], &salt), pl = unhex(argv[3], &pw);
        uint8_t v[256];
        if (ccsrp_generate_verifier(ctx, "", pl, pw, sl, salt, v)) return 2;
        hex("v", v, sizeof v);
        return 0;
    }
    if ((argc == 6 || argc == 7) && !strcmp(argv[1], "client")) {
        size_t sl = unhex(argv[2], &salt);
        unhex(argv[3], &B);
        size_t pl = unhex(argv[4], &pw);
        seed = (uint32_t)strtoul(argv[5], NULL, 16);
        struct ccrng_state rng = {det_generate};
        uint8_t A[256], M[64];
        if (ccsrp_client_start_authentication(ctx, &rng, A)) return 3;
        hex("A", A, sizeof A);
        int rc = ccsrp_client_process_challenge(ctx, "", pl, pw, sl, salt, B, M);
        if (rc) { printf("process_challenge=%d\n", rc); return 4; }
        hex("M1", M, sizeof M);
        size_t kl = 0;
        const void *K = ccsrp_get_session_key(ctx, &kl);
        hex("K", K, kl);
        if (argc == 7) {
            unhex(argv[6], &m2);
            printf("verify=%d\n", ccsrp_client_verify_session(ctx, m2) ? 1 : 0);
        }
        return 0;
    }
    fprintf(stderr, "usage: see source\n");
    return 1;
}
