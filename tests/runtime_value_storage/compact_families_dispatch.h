static sv4_t evaluate(unsigned op, sv4_t a, sv4_t b, sv4_t c) {
    switch (op) {
    case 0:
        return sv4_shl(a, b);
    case 1:
        return sv4_shr(a, b);
    case 2:
        return sv4_ashl(a, b);
    case 3:
        return sv4_ashr(a, b);
    case 4:
        return sv4_reduce_and(a);
    case 5:
        return sv4_reduce_nand(a);
    case 6:
        return sv4_reduce_or(a);
    case 7:
        return sv4_reduce_nor(a);
    case 8:
        return sv4_reduce_xor(a);
    case 9:
        return sv4_reduce_xnor(a);
    case 10:
        return sv4_countones(a);
    case 11:
        return sv4_onehot(a, 0);
    case 12:
        return sv4_onehot(a, 1);
    case 13:
        return sv4_casex_eq(a, b);
    case 14:
        return sv4_casez_eq(a, b);
    case 15:
        return sv4_wild_eq(a, b);
    case 16:
        return sv4_wild_neq(a, b);
    case 17:
        return sv4_logimpl(a, b);
    case 18:
        return sv4_logequiv(a, b);
    case 19:
        return sv4_inside_range(a, b, c);
    default:
        abort();
    }
}
