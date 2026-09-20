#include <stdlib.h>

/* C23 string conversion compatibility stubs for glibc < 2.38 */

long __isoc23_strtol(const char *nptr, char **endptr, int base) {
    return strtol(nptr, endptr, base);
}

long long __isoc23_strtoll(const char *nptr, char **endptr, int base) {
    return strtoll(nptr, endptr, base);
}

unsigned long __isoc23_strtoul(const char *nptr, char **endptr, int base) {
    return strtoul(nptr, endptr, base);
}

unsigned long long __isoc23_strtoull(const char *nptr, char **endptr, int base) {
    return strtoull(nptr, endptr, base);
}

double __isoc23_strtod(const char *nptr, char **endptr) {
    return strtod(nptr, endptr);
}

float __isoc23_strtof(const char *nptr, char **endptr) {
    return strtof(nptr, endptr);
}
