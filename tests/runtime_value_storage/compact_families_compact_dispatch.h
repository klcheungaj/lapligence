static g4_t compact_eval(unsigned op, g4_t a, g4_t b, g4_t c) {
    switch (op) {
    case 0:
        return llg_gmp_sv4_shl(a, b);
    case 1:
        return llg_gmp_sv4_shr(a, b);
    case 2:
        return llg_gmp_sv4_ashl(a, b);
    case 3:
        return llg_gmp_sv4_ashr(a, b);
    case 4:
        return llg_gmp_sv4_reduce_and(a);
    case 5:
        return llg_gmp_sv4_reduce_nand(a);
    case 6:
        return llg_gmp_sv4_reduce_or(a);
    case 7:
        return llg_gmp_sv4_reduce_nor(a);
    case 8:
        return llg_gmp_sv4_reduce_xor(a);
    case 9:
        return llg_gmp_sv4_reduce_xnor(a);
    case 10:
        return llg_gmp_sv4_countones(a);
    case 11:
        return llg_gmp_sv4_onehot(a, 0);
    case 12:
        return llg_gmp_sv4_onehot(a, 1);
    case 13:
        return llg_gmp_sv4_casex_eq(a, b);
    case 14:
        return llg_gmp_sv4_casez_eq(a, b);
    case 15:
        return llg_gmp_sv4_wild_eq(a, b);
    case 16:
        return llg_gmp_sv4_wild_neq(a, b);
    case 17:
        return llg_gmp_sv4_logimpl(a, b);
    case 18:
        return llg_gmp_sv4_logequiv(a, b);
    case 19:
        return llg_gmp_sv4_inside_range(a, b, c);
    default:
        abort();
    }
}
