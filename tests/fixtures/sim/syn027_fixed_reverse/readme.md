# SYN-027 fixed reverse

The focused suite also runs the existing `data_types_completion/syn_027_fixed_reverse.sv`
fixture through the public CLI for lengths 1/2/3/17, reversed and negative
bounds, packed records, automatic locals, and selected rows.

`notifications.sv` checks that selected three-dimensional rows and unpacked
record elements reverse as whole immediate elements. It checks one receiver
selection per call, untouched neighboring rows, reverse twice, and settled
continuous/`always_comb` consumers after each in-place permutation.

`formal_publication.sv` checks selected automatic input, inout and forwarded ref
receivers, including private input mutation, copy back, one selection per call,
unchanged neighbors and publication to a continuous reader after the ref write.

`edition_boundary.sv` uses Verilog-2001-compatible declarations around the
SystemVerilog array method call. It runs in SV2009 and must reject in 2001.
Expected element order follows IEEE 1800-2009 §7.12.2; the method is absent
from IEEE 1364-2001.

`unselected_real.sv` is a legal SystemVerilog method use excluded by llg's
fixed integral scope; rejection is a supported-profile boundary, not an IEEE
syntax error. Existing completion fixtures separately check illegal `with`
and read-only const-ref receivers.
