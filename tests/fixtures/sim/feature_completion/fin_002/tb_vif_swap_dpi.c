/* FIN-002 DPI-C companion of tb_vif_swap.sv: an order-dependent fold of the
 * values the monitor saw. */
int c_fold(int folded, int value)
{
    return folded * 31 + value;
}
